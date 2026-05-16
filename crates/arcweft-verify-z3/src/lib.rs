//! External Z3 process adapter for Arcweft verification.
//!
//! This crate intentionally keeps Z3 outside the verifier core. The adapter
//! receives solver-neutral SMT problems, emits SMT-LIB, and invokes a configured
//! executable from CLI or tests.

use arcweft_verify::{SmtBackend, SmtError, SmtOutcome, SmtProblem, emit_smt_lib};
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Errors specific to the external Z3 process adapter.
#[derive(Debug, Error)]
pub enum Z3AdapterError {
    #[error("failed to spawn z3 process: {0}")]
    Spawn(std::io::Error),
    #[error("failed to write SMT-LIB to z3 stdin: {0}")]
    Write(std::io::Error),
    #[error("failed to wait for z3 process: {0}")]
    Wait(std::io::Error),
    #[error("z3 exited with status {status}: {stderr}")]
    Status { status: String, stderr: String },
    #[error("z3 returned an unrecognized result: {0}")]
    UnknownOutput(String),
}

/// Adapter that invokes an external Z3-compatible command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalZ3Backend {
    command: OsString,
}

impl ExternalZ3Backend {
    /// Creates a backend using the provided executable name or path.
    pub fn new(command: impl Into<OsString>) -> Self {
        Self {
            command: command.into(),
        }
    }

    /// Checks an SMT problem and exposes adapter-specific errors.
    pub fn check_problem(&self, problem: &SmtProblem) -> Result<SmtOutcome, Z3AdapterError> {
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
            .write_all(emit_smt_lib(problem).as_bytes())
            .map_err(Z3AdapterError::Write)?;
        drop(stdin);

        let output = child.wait_with_output().map_err(Z3AdapterError::Wait)?;
        if !output.status.success() {
            return Err(Z3AdapterError::Status {
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        parse_z3_stdout(&String::from_utf8_lossy(&output.stdout))
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

    fn check(&self, problem: &SmtProblem) -> Result<SmtOutcome, SmtError> {
        self.check_problem(problem)
            .map_err(|error| SmtError::new(error.to_string()))
    }
}

fn parse_z3_stdout(stdout: &str) -> Result<SmtOutcome, Z3AdapterError> {
    match stdout.lines().next().map(str::trim) {
        Some("sat") => Ok(SmtOutcome::Sat),
        Some("unsat") => Ok(SmtOutcome::Unsat),
        Some("unknown") => Ok(SmtOutcome::Unknown),
        other => Err(Z3AdapterError::UnknownOutput(
            other.unwrap_or_default().to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_solver_output() {
        assert_eq!(parse_z3_stdout("sat\n").expect("sat"), SmtOutcome::Sat);
        assert_eq!(
            parse_z3_stdout("unsat\n").expect("unsat"),
            SmtOutcome::Unsat
        );
        assert_eq!(
            parse_z3_stdout("unknown\n").expect("unknown"),
            SmtOutcome::Unknown
        );
    }
}
