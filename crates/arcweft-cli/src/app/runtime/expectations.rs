use crate::output::RuntimeStepRunSummary;
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal, parse_expr};
use arcweft_runtime_host::NativeTaskBridge;
use arcweft_test::{ScriptStep, ScriptTest};
use std::path::Path;

pub(in crate::app) fn test_start_flow(test: &ScriptTest) -> Option<String> {
    test.steps
        .iter()
        .find_map(|step| parse_start_flow_call(&step.text))
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
    source_path: Option<&'a Path>,
}

impl<'a> RuntimeExpectationView<'a> {
    pub(in crate::app) const fn new(frames: &'a [RuntimeStepRunSummary]) -> Self {
        Self {
            frames,
            source_path: None,
        }
    }

    pub(in crate::app) const fn with_source_path(
        frames: &'a [RuntimeStepRunSummary],
        source_path: &'a Path,
    ) -> Self {
        Self {
            frames,
            source_path: Some(source_path),
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
        let Some(source_path) = self.source_path else {
            return Err("file expectations require a source-backed runtime".to_owned());
        };
        NativeTaskBridge::read_text_snapshot(source_path, virtual_path)
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

pub(in crate::app) fn parse_start_flow_call(text: &str) -> Option<String> {
    let Expr::Call { callee, args } = parse_expr(text).ok()? else {
        return None;
    };
    let Expr::Path(name) = callee.as_ref() else {
        return None;
    };
    if name != "start" {
        return None;
    }
    let [flow] = args.as_slice() else {
        return None;
    };
    entity_ref_label(flow.value())
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
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(path) if path == "log") => {
            field.clone()
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
    let Expr::MethodCall {
        receiver,
        method,
        args,
    } = parse_expr(text).ok()?
    else {
        return None;
    };
    matches!(receiver.as_ref(), Expr::Path(path) if path == "expect").then_some((method, args))
}

fn virtual_path_label(expr: &Expr) -> Option<String> {
    let Expr::MethodCall {
        receiver,
        method,
        args,
    } = expr
    else {
        return None;
    };
    if !matches!(receiver.as_ref(), Expr::Path(path) if path == "path") {
        return None;
    }
    if !matches!(method.as_str(), "save" | "asset" | "temp" | "export") {
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

fn entity_ref_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        _ => None,
    }
}

fn expectation_value_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(format!("@{}", entity.body())),
        Expr::Path(path) => Some(path.clone()),
        Expr::Literal(Literal::Bool(value)) => Some(value.to_string()),
        Expr::Literal(Literal::Int { value, .. }) => Some(value.to_string()),
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
