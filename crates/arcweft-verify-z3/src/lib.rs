//! External Z3 process adapter for Arcweft verification.
//!
//! The adapter receives a fully declared [`SmtProblem`], invokes a configured
//! executable, and extracts requested counterexample values. It does not define
//! a second expression enum or backend trait.

use arcweft_verify::smt::{SmtBackend, SmtCheck, SmtEmission, SmtError, SmtOutcome, SmtProblem};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Errors specific to the external Z3 process adapter.
#[derive(Debug, Error)]
pub enum Z3AdapterError {
    #[error("invalid SMT problem `{problem}`: {message}")]
    InvalidProblem { problem: String, message: String },
    #[error("failed to spawn z3 process: {0}")]
    Spawn(std::io::Error),
    #[error("failed to write SMT-LIB to z3 stdin: {0}")]
    Write(std::io::Error),
    #[error("failed to wait for z3 process: {0}")]
    Wait(std::io::Error),
    #[error("z3 exited with status {status}: {stderr}")]
    Status { status: String, stderr: String },
    #[error("z3 reported an SMT-LIB error: {0}")]
    Solver(String),
    #[error("z3 returned no sat/unsat/unknown result: {0}")]
    UnknownOutput(String),
    #[error("failed to parse z3 counterexample model: {0}")]
    Model(String),
}

/// Adapter that invokes an external Z3-compatible command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalZ3Backend {
    command: OsString,
}

impl ExternalZ3Backend {
    pub fn new(command: impl Into<OsString>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn check_problem(&self, problem: &SmtProblem) -> Result<SmtCheck, Z3AdapterError> {
        let check_script = problem
            .emit_smt_lib(SmtEmission::CheckOnly)
            .map_err(|error| Z3AdapterError::InvalidProblem {
                problem: problem.name.clone(),
                message: error.to_string(),
            })?;
        let check_output = self.run_script(&check_script)?;
        let outcome = SmtOutcome::from_solver_output(&check_output)
            .ok_or_else(|| Z3AdapterError::UnknownOutput(check_output.clone()))?;
        if !outcome.is_counterexample() || problem.model_symbols.is_empty() {
            return Ok(SmtCheck::new(outcome).with_raw_output(check_output));
        }

        let model_script = problem
            .emit_smt_lib(SmtEmission::CounterexampleValues)
            .map_err(|error| Z3AdapterError::InvalidProblem {
                problem: problem.name.clone(),
                message: error.to_string(),
            })?;
        let model_output = self.run_script(&model_script)?;
        if SmtOutcome::from_solver_output(&model_output) != Some(SmtOutcome::Sat) {
            return Err(Z3AdapterError::UnknownOutput(model_output));
        }
        let model =
            SExpression::parse_get_value_output(&model_output).map_err(Z3AdapterError::Model)?;
        Ok(SmtCheck::new(outcome)
            .with_model(model)
            .with_raw_output(format!(
                "{check_output}; counterexample value pass\n{model_output}"
            )))
    }

    fn run_script(&self, script: &str) -> Result<String, Z3AdapterError> {
        let mut child = Command::new(&self.command)
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Z3AdapterError::Spawn)?;

        let Some(mut stdin) = child.stdin.take() else {
            return Err(Z3AdapterError::Write(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "z3 stdin was not available",
            )));
        };
        stdin
            .write_all(script.as_bytes())
            .map_err(Z3AdapterError::Write)?;
        drop(stdin);

        let output = child.wait_with_output().map_err(Z3AdapterError::Wait)?;
        if !output.status.success() {
            return Err(Z3AdapterError::Status {
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if stdout
            .lines()
            .chain(stderr.lines())
            .any(|line| line.trim_start().starts_with("(error"))
        {
            return Err(Z3AdapterError::Solver(format!(
                "stdout:\n{stdout}\nstderr:\n{stderr}"
            )));
        }
        Ok(stdout)
    }
}

impl Default for ExternalZ3Backend {
    fn default() -> Self {
        Self::new("z3")
    }
}

impl SmtBackend for ExternalZ3Backend {
    fn name(&self) -> &'static str {
        "z3"
    }

    fn check(&self, problem: &SmtProblem) -> Result<SmtCheck, SmtError> {
        self.check_problem(problem)
            .map_err(|error| SmtError::new(error.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SExpression {
    Atom(String),
    List(Vec<Self>),
}

impl SExpression {
    fn parse_get_value_output(output: &str) -> Result<BTreeMap<String, String>, String> {
        let Some(result_offset) = output
            .lines()
            .position(|line| matches!(line.trim(), "sat" | "unsat" | "unknown"))
        else {
            return Err("solver outcome line is missing".to_owned());
        };
        let payload = output
            .lines()
            .skip(result_offset + 1)
            .collect::<Vec<_>>()
            .join("\n");
        if payload.trim().is_empty() {
            return Ok(BTreeMap::new());
        }

        let expressions = Self::parse_many(&payload)?;
        let pairs = expressions
            .iter()
            .find_map(Self::model_pairs)
            .ok_or_else(|| format!("no get-value pair list in `{}`", payload.trim()))?;
        Ok(pairs
            .iter()
            .filter_map(Self::model_pair)
            .collect::<BTreeMap<_, _>>())
    }

    fn parse_many(input: &str) -> Result<Vec<Self>, String> {
        let tokens = Token::tokenize(input)?;
        let mut cursor = 0;
        let mut expressions = Vec::new();
        while cursor < tokens.len() {
            expressions.push(Self::parse_one(&tokens, &mut cursor)?);
        }
        Ok(expressions)
    }

    fn parse_one(tokens: &[Token], cursor: &mut usize) -> Result<Self, String> {
        let Some(token) = tokens.get(*cursor) else {
            return Err("unexpected end of S-expression".to_owned());
        };
        *cursor += 1;
        match token {
            Token::Atom(value) => Ok(Self::Atom(value.clone())),
            Token::Open => {
                let mut items = Vec::new();
                loop {
                    match tokens.get(*cursor) {
                        Some(Token::Close) => {
                            *cursor += 1;
                            return Ok(Self::List(items));
                        }
                        Some(_) => items.push(Self::parse_one(tokens, cursor)?),
                        None => return Err("unterminated S-expression list".to_owned()),
                    }
                }
            }
            Token::Close => Err("unexpected `)`".to_owned()),
        }
    }

    fn model_pairs(&self) -> Option<&[Self]> {
        let Self::List(items) = self else {
            return None;
        };
        if items.iter().all(|item| {
            matches!(
                item,
                Self::List(pair)
                    if pair.len() == 2 && matches!(pair.first(), Some(Self::Atom(_)))
            )
        }) {
            Some(items)
        } else {
            items.iter().find_map(Self::model_pairs)
        }
    }

    fn model_pair(&self) -> Option<(String, String)> {
        let Self::List(items) = self else {
            return None;
        };
        let [Self::Atom(symbol), value] = items.as_slice() else {
            return None;
        };
        Some((Self::unquote_symbol(symbol), value.render_model_value()))
    }

    fn unquote_symbol(symbol: &str) -> String {
        symbol
            .strip_prefix('|')
            .and_then(|value| value.strip_suffix('|'))
            .unwrap_or(symbol)
            .to_owned()
    }

    fn render_model_value(&self) -> String {
        if let Self::List(items) = self
            && let [Self::Atom(operator), Self::Atom(value)] = items.as_slice()
            && operator == "-"
            && value.chars().all(|character| character.is_ascii_digit())
        {
            return format!("-{value}");
        }
        self.render()
    }

    fn render(&self) -> String {
        match self {
            Self::Atom(value) => value.clone(),
            Self::List(items) => format!(
                "({})",
                items.iter().map(Self::render).collect::<Vec<_>>().join(" ")
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Open,
    Close,
    Atom(String),
}

impl Token {
    fn tokenize(input: &str) -> Result<Vec<Self>, String> {
        let mut tokens = Vec::new();
        let mut characters = input.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '(' => tokens.push(Self::Open),
                ')' => tokens.push(Self::Close),
                ';' => {
                    for character in characters.by_ref() {
                        if character == '\n' {
                            break;
                        }
                    }
                }
                character if character.is_whitespace() => {}
                '|' => {
                    let mut atom = String::from("|");
                    let mut closed = false;
                    for character in characters.by_ref() {
                        atom.push(character);
                        if character == '|' {
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return Err("unterminated quoted symbol".to_owned());
                    }
                    tokens.push(Self::Atom(atom));
                }
                first => {
                    let mut atom = String::from(first);
                    while let Some(next) = characters.peek() {
                        if next.is_whitespace() || matches!(*next, '(' | ')' | ';') {
                            break;
                        }
                        atom.push(*next);
                        characters.next();
                    }
                    tokens.push(Self::Atom(atom));
                }
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::SExpression;
    use arcweft_verify::smt::SmtOutcome;

    #[test]
    fn outcome_parser_is_owned_by_smt_outcome() {
        assert_eq!(
            SmtOutcome::from_solver_output("success\nunsat\n"),
            Some(SmtOutcome::Unsat)
        );
    }

    #[test]
    fn parses_boolean_and_integer_model_values() {
        let model = SExpression::parse_get_value_output(
            "sat\n((before 100)\n (delta (- 1))\n (selected true))\n",
        )
        .expect("model parses");
        assert_eq!(model.get("before").map(String::as_str), Some("100"));
        assert_eq!(model.get("delta").map(String::as_str), Some("-1"));
        assert_eq!(model.get("selected").map(String::as_str), Some("true"));
    }
}
