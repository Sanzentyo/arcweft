use crate::output::RuntimeStepRunSummary;
use arcweft_runtime_host::{NativeFileRoots, NativeTaskBridge};
use arcweft_test::{ScriptCommand, ScriptExpectation, ScriptTest};

pub(in crate::app) fn test_goto_flow(test: &ScriptTest) -> Option<String> {
    test.steps.iter().find_map(|step| match &step.command {
        ScriptCommand::Goto { target } => Some(target.clone()),
        ScriptCommand::Expectation { .. }
        | ScriptCommand::Pure { .. }
        | ScriptCommand::Scope { .. }
        | ScriptCommand::Other { .. } => None,
    })
}

pub(in crate::app) fn test_expectation_failures(
    test: &ScriptTest,
    frames: &[RuntimeStepRunSummary],
) -> Vec<String> {
    let observations = RuntimeExpectationView::new(frames);
    test.steps
        .iter()
        .filter_map(|step| match &step.command {
            ScriptCommand::Expectation { expectation } => {
                evaluate_runtime_expectation(expectation, &observations).err()
            }
            ScriptCommand::Goto { .. }
            | ScriptCommand::Pure { .. }
            | ScriptCommand::Scope { .. }
            | ScriptCommand::Other { .. } => None,
        })
        .collect()
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
    expectation: &ScriptExpectation,
    observations: &RuntimeExpectationView<'_>,
) -> Result<(), String> {
    match expectation {
        ScriptExpectation::NoAssertionFailures => {
            if observations
                .frames()
                .iter()
                .all(|frame| frame.assertion_failures.is_empty())
            {
                Ok(())
            } else {
                Err("expected no runtime assertion failures".to_owned())
            }
        }
        ScriptExpectation::Signal { target, expected } => {
            let actual = observations.signal_value(target);
            if actual == Some(expected.as_str()) {
                Ok(())
            } else {
                Err(format!(
                    "expected signal {target} == {expected}, found {}",
                    actual.unwrap_or("<missing>")
                ))
            }
        }
        ScriptExpectation::Log { level, contains } => {
            if observations.has_log(level, contains) {
                Ok(())
            } else {
                Err(format!("expected log.{level} containing `{contains}`"))
            }
        }
        ScriptExpectation::File { path, equals } => {
            let virtual_path = path.runtime_label();
            let actual = observations.file_text(&virtual_path)?;
            if actual == *equals {
                Ok(())
            } else {
                Err(format!(
                    "expected file {virtual_path} == `{equals}`, found `{actual}`"
                ))
            }
        }
        ScriptExpectation::Unsupported { method } => Err(format!(
            "unsupported runtime expectation `{}`",
            method.as_deref().unwrap_or("<invalid-call>")
        )),
    }
}
