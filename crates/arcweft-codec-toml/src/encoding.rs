use std::fmt::{self, Write as _};

use arcweft_data::{DataError, DataErrorKind, EncodeBuffer, Result};
use toml::Value;

/// Emits the already shape-checked TOML model directly into the bounded output.
/// Inline tables preserve the complete table/array algebra without constructing
/// the upstream serializer's unbounded encoded table-body strings.
pub(super) fn write_document(value: &Value, output: &mut EncodeBuffer) -> Result<()> {
    let Value::Table(table) = value else {
        return Err(DataError::invalid_type("TOML root table", value.type_str()));
    };
    table
        .iter()
        .try_for_each(|(key, value)| {
            write_key(output, key)?;
            output.write_str(" = ")?;
            write_value(output, value)?;
            output.write_char('\n')
        })
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
}

fn write_key(output: &mut EncodeBuffer, key: &str) -> fmt::Result {
    if !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        output.write_str(key)
    } else {
        write_string(output, key)
    }
}

fn write_string(output: &mut EncodeBuffer, value: &str) -> fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            '\u{08}' => output.write_str("\\b")?,
            '\u{0c}' => output.write_str("\\f")?,
            '\u{00}'..='\u{1f}' | '\u{7f}' => write!(output, "\\u{:04X}", u32::from(character))?,
            character => output.write_char(character)?,
        }
    }
    output.write_char('"')
}

fn write_value(output: &mut EncodeBuffer, value: &Value) -> fmt::Result {
    match value {
        Value::String(value) => write_string(output, value),
        Value::Integer(value) => write!(output, "{value}"),
        Value::Float(value) if value.is_nan() => output.write_str("nan"),
        Value::Float(value) => write!(output, "{value:?}"),
        Value::Boolean(value) => write!(output, "{value}"),
        Value::Datetime(value) => write!(output, "{value}"),
        Value::Array(values) => {
            output.write_char('[')?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                write_value(output, value)?;
            }
            output.write_char(']')
        }
        Value::Table(table) => {
            output.write_char('{')?;
            for (index, (key, value)) in table.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                write_key(output, key)?;
                output.write_str(" = ")?;
                write_value(output, value)?;
            }
            output.write_char('}')
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, write_document};
    use arcweft_data::EncodeOptions;

    #[test]
    fn bounded_toml_preserves_nested_tables_arrays_scalars_and_escaped_keys() {
        let source = r#"
"" = "\u0000\u007F\b\f\t\r\n\"\\蟹"
"a.b" = { "quoted key" = [1, 2, 3], empty = {}, rows = [{ name = "x" }, { name = "y" }] }
floats = [0.0, -0.0, 1e100, -1e-100, inf, -inf]
date = 1979-05-27T07:32:00Z
bool = true
        "#;
        let expected: Value = toml::from_str(source).unwrap();
        let bytes = EncodeOptions::default()
            .encode_with(|output| write_document(&expected, output))
            .unwrap();
        let actual: Value = toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(actual, expected);
        let nan: Value = toml::from_str("value = nan").unwrap();
        let bytes = EncodeOptions::default()
            .encode_with(|output| write_document(&nan, output))
            .unwrap();
        let actual: Value = toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert!(actual["value"].as_float().unwrap().is_nan());
    }
}
