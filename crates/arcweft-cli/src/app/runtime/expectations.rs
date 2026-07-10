use crate::output::RuntimeStepRunSummary;
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal, parse_expr};
use arcweft_runtime_host::{NativeFileRoots, NativeTaskBridge};
use arcweft_test::{ScriptStep, ScriptTest};

pub(in crate::app) fn test_goto_flow(test: &ScriptTest) -> Option<String> {
    test.steps
        .iter()
        .find_map(|step| parse_goto_flow_in_text(&step.text))
}

pub(in crate::app) fn test_expectation_failures(
    test: &ScriptTest,
    frames: &[RuntimeStepRunSummary],
) -> Vec<String> {
    test.steps
        .iter()
        .filter(|step| step.command == "expect" || step.command.starts_with("expect."))
        .filter_map(|step| evaluate_test_expectation(step, frames).err())
        .collect()
}

fn evaluate_test_expectation(
    step: &ScriptStep,
    frames: &[RuntimeStepRunSummary],
) -> Result<(), String> {
    evaluate_runtime_expectation(step.text.trim(), &RuntimeExpectationView::new(frames))
}

pub(in crate::app) struct RuntimeExpectationView<'a> {
    frames: &'a [RuntimeStepRunSummary],
    file_roots: Option<&'a NativeFileRoots>,
}

impl<'a> RuntimeExpectationView<'a> {
    pub(in crate::app) const fn new(frames: &'a [RuntimeStepRunSummary]) -> Self {
        Self {
            frames,
            file_roots: None,
        }
    }

    pub(in crate::app) const fn with_file_roots(
        frames: &'a [RuntimeStepRunSummary],
        file_roots: &'a NativeFileRoots,
    ) -> Self {
        Self {
            frames,
            file_roots: Some(file_roots),
        }
    }

    fn frames(&self) -> &[RuntimeStepRunSummary] {
        self.frames
    }

    fn signal_value(&self, target: &str) -> Option<&str> {
        self.frames
            .last()?
            .observations
            .signals
            .iter()
            .find(|signal| signal.target == target)
            .map(|signal| signal.value.as_str())
    }

    fn has_log(&self, level: &str, needle: &str) -> bool {
        self.frames.last().is_some_and(|frame| {
            frame
                .observations
                .logs
                .iter()
                .any(|log| log.level == level && log.message.contains(needle))
        })
    }

    fn file_text(&self, virtual_path: &str) -> Result<String, String> {
        let Some(file_roots) = self.file_roots else {
            return Err("file expectations require a source-backed runtime".to_owned());
        };
        NativeTaskBridge::read_text_snapshot(file_roots, virtual_path)
    }
}

pub(in crate::app) fn evaluate_runtime_expectation(
    text: &str,
    observations: &RuntimeExpectationView<'_>,
) -> Result<(), String> {
    if is_expect_no_assertion_failures_call(text) {
        if observations
            .frames()
            .iter()
            .all(|frame| frame.diagnostics.is_empty())
        {
            return Ok(());
        }
        return Err("expected no assertion/runtime diagnostics".to_owned());
    }
    if let Some((target, expected)) = parse_expect_signal_call(text) {
        let actual = observations.signal_value(&target);
        if actual == Some(expected.as_str()) {
            return Ok(());
        }
        return Err(format!(
            "expected signal {target} == {expected}, found {}",
            actual.unwrap_or("<missing>")
        ));
    }
    if let Some((level, needle)) = parse_expect_log_call(text) {
        if observations.has_log(&level, &needle) {
            return Ok(());
        }
        return Err(format!("expected log.{level} containing `{needle}`"));
    }
    if let Some((virtual_path, expected)) = parse_expect_file_call(text) {
        let actual = observations.file_text(&virtual_path)?;
        if actual == expected {
            return Ok(());
        }
        return Err(format!(
            "expected file {virtual_path} == `{expected}`, found `{actual}`"
        ));
    }
    Err(format!("unsupported runtime expectation `{text}`"))
}

pub(in crate::app) fn parse_goto_flow_statement(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("goto")?;
    if !rest.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let target = rest.trim();
    if !target.starts_with('@') || target.contains(char::is_whitespace) {
        return None;
    }
    target.strip_prefix('@').map(str::to_owned)
}

pub(in crate::app) fn parse_goto_flow_in_text(text: &str) -> Option<String> {
    text.lines()
        .flat_map(|line| line.split(['{', ';', '}']))
        .find_map(parse_goto_flow_statement)
}

fn parse_expect_signal_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "signal" {
        return None;
    }
    let [target, expected] = args.as_slice() else {
        return None;
    };
    Some((
        expectation_value_label(target.value())?,
        expectation_value_label(expected.value())?,
    ))
}

fn is_expect_no_assertion_failures_call(text: &str) -> bool {
    parse_expect_method_call(text)
        .is_some_and(|(method, args)| method == "no_assertion_failures" && args.is_empty())
}

fn parse_expect_log_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "log" {
        return None;
    }
    let [level, contains] = args.as_slice() else {
        return None;
    };
    let level = match level.value() {
        Expr::Path(path) => path.trim_start_matches('.').to_owned(),
        Expr::Select(select) if matches!(select.target(), Expr::Path(path) if path == "log") => {
            select.member().as_str().to_owned()
        }
        _ => return None,
    };
    let CallArg::Named { name, value } = contains else {
        return None;
    };
    if name != "contains" {
        return None;
    }
    Some((level, string_literal_value(value)?))
}

fn parse_expect_file_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "file" {
        return None;
    }
    let [path, expected] = args.as_slice() else {
        return None;
    };
    let CallArg::Named { name, value } = expected else {
        return None;
    };
    if name != "equals" {
        return None;
    }
    Some((
        virtual_path_label(path.value())?,
        string_literal_value(value)?,
    ))
}

fn parse_expect_method_call(text: &str) -> Option<(String, Vec<CallArg>)> {
    let Expr::Call { callee, args } = parse_expr(text).ok()? else {
        return None;
    };
    let Expr::Select(select) = callee.as_ref() else {
        return None;
    };
    matches!(select.target(), Expr::Path(path) if path == "expect")
        .then_some((select.member().as_str().to_owned(), args))
}

fn virtual_path_label(expr: &Expr) -> Option<String> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let Expr::Select(select) = callee.as_ref() else {
        return None;
    };
    if !matches!(select.target(), Expr::Path(path) if path == "path") {
        return None;
    }
    let method = select.member().as_str();
    if !matches!(method, "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [relative] = args.as_slice() else {
        return None;
    };
    Some(format!(
        "{method}:{}",
        string_literal_value(relative.value())?
    ))
}

fn expectation_value_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(format!("@{}", entity.body())),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::ShortVariant(name) => Some(format!(".{name}")),
        Expr::Literal(Literal::Bool(value)) => Some(value.to_string()),
        Expr::Literal(Literal::Int(literal)) => Some(literal.raw().to_owned()),
        Expr::Literal(Literal::Float { raw, .. } | Literal::String(raw)) => Some(raw.clone()),
        _ => None,
    }
}

fn string_literal_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}
