use super::options::{CliRuntimePureWorkers, CliRuntimeStepMode};
use arcweft_core::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
use arcweft_core::value::{RuntimeBinding, RuntimeValue, runtime_sequence_dense_f32};

pub(in crate::app) fn parse_runtime_binding_arg(value: &str) -> Result<RuntimeBinding, String> {
    let Some((name, raw)) = value.split_once('=') else {
        return Err("expected name=value".to_owned());
    };
    if name.is_empty() {
        return Err("binding name must not be empty".to_owned());
    }
    Ok(RuntimeBinding {
        name: name.to_owned(),
        value: parse_runtime_value(raw)?,
    })
}

fn parse_runtime_value(raw: &str) -> Result<RuntimeValue, String> {
    match raw {
        "true" => Ok(RuntimeValue::Bool(true)),
        "false" => Ok(RuntimeValue::Bool(false)),
        "()" => Ok(RuntimeValue::Unit),
        value if value.starts_with("matrix/f32/") => parse_runtime_matrix_f32(value),
        value if value.starts_with("matrix/f64/") => parse_runtime_matrix_f64(value),
        value if value.starts_with("tensor/f32/") => parse_runtime_tensor_f32(value),
        value if value.starts_with("tensor/f64/") => parse_runtime_tensor_f64(value),
        value if value.starts_with("seq/f32:") => parse_runtime_f32_sequence(value),
        value if value.starts_with('@') => Ok(RuntimeValue::EntityRef(value[1..].to_owned())),
        value => value
            .parse::<i64>()
            .map(RuntimeValue::i64)
            .or_else(|_| Ok(RuntimeValue::String(value.to_owned()))),
    }
}

fn parse_runtime_matrix_f32(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("matrix/f32/")
        .split_once(':')
        .ok_or_else(|| "matrix/f32 value must be matrix/f32/<rows>x<cols>:<csv>".to_owned())?;
    let (rows, cols) = shape
        .split_once('x')
        .ok_or_else(|| "matrix/f32 shape must be <rows>x<cols>".to_owned())?;
    let rows = parse_nonzero_usize(rows, "matrix/f32 rows")?;
    let cols = parse_nonzero_usize(cols, "matrix/f32 cols")?;
    let values = parse_f32_csv(values, "matrix/f32")?;
    DenseMatrixF32::new(rows, cols, values)
        .map(RuntimeValue::MatrixF32)
        .map_err(|error| error.to_string())
}

fn parse_runtime_tensor_f32(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("tensor/f32/")
        .split_once(':')
        .ok_or_else(|| "tensor/f32 value must be tensor/f32/<dims>:<csv>".to_owned())?;
    let dims = shape
        .split('x')
        .map(|dim| parse_nonzero_usize(dim, "tensor/f32 dim"))
        .collect::<Result<Vec<_>, _>>()?;
    let values = parse_f32_csv(values, "tensor/f32")?;
    DenseTensorF32::new(dims, values)
        .map(RuntimeValue::TensorF32)
        .map_err(|error| error.to_string())
}

fn parse_runtime_matrix_f64(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("matrix/f64/")
        .split_once(':')
        .ok_or_else(|| "matrix/f64 value must be matrix/f64/<rows>x<cols>:<csv>".to_owned())?;
    let (rows, cols) = shape
        .split_once('x')
        .ok_or_else(|| "matrix/f64 shape must be <rows>x<cols>".to_owned())?;
    let rows = parse_nonzero_usize(rows, "matrix/f64 rows")?;
    let cols = parse_nonzero_usize(cols, "matrix/f64 cols")?;
    let values = parse_f64_csv(values, "matrix/f64")?;
    DenseMatrixF64::new(rows, cols, values)
        .map(RuntimeValue::MatrixF64)
        .map_err(|error| error.to_string())
}

fn parse_runtime_tensor_f64(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("tensor/f64/")
        .split_once(':')
        .ok_or_else(|| "tensor/f64 value must be tensor/f64/<dims>:<csv>".to_owned())?;
    let dims = shape
        .split('x')
        .map(|dim| parse_nonzero_usize(dim, "tensor/f64 dim"))
        .collect::<Result<Vec<_>, _>>()?;
    let values = parse_f64_csv(values, "tensor/f64")?;
    DenseTensorF64::new(dims, values)
        .map(RuntimeValue::TensorF64)
        .map_err(|error| error.to_string())
}

fn parse_runtime_f32_sequence(raw: &str) -> Result<RuntimeValue, String> {
    let values = raw
        .strip_prefix("seq/f32:")
        .ok_or_else(|| "not an f32 sequence".to_owned())
        .and_then(|values| parse_f32_csv(values, "seq/f32"))?;
    Ok(runtime_sequence_dense_f32(values))
}

fn parse_nonzero_usize(raw: &str, label: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("{label} must be a positive integer, got `{raw}`"))?;
    if value == 0 {
        return Err(format!("{label} must be greater than zero"));
    }
    Ok(value)
}

fn parse_f32_csv(raw: &str, label: &str) -> Result<Vec<f32>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("{label} element must be f32, got `{value}`"))
        })
        .collect()
}

fn parse_f64_csv(raw: &str, label: &str) -> Result<Vec<f64>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{label} element must be f64, got `{value}`"))
        })
        .collect()
}

pub(in crate::app) fn parse_runtime_pure_workers(
    raw: &str,
) -> Result<CliRuntimePureWorkers, String> {
    if raw == "auto" {
        return Ok(CliRuntimePureWorkers::Auto);
    }
    let value = raw.parse::<usize>().map_err(|_| {
        format!("pure worker count must be `auto` or a positive integer, got `{raw}`")
    })?;
    if value == 0 {
        return Err("pure worker count must be greater than zero".to_owned());
    }
    Ok(CliRuntimePureWorkers::Fixed(value))
}

pub(in crate::app) fn step_options(mode: CliRuntimeStepMode, max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: mode.into(),
        budget: RuntimeStepBudget { max_ops },
    }
}

impl From<CliRuntimeStepMode> for RuntimeStepMode {
    fn from(value: CliRuntimeStepMode) -> Self {
        match value {
            CliRuntimeStepMode::OneOp => Self::OneOp,
            CliRuntimeStepMode::Drain => Self::Drain,
            CliRuntimeStepMode::Game => Self::Game,
            CliRuntimeStepMode::Server => Self::Server,
        }
    }
}
