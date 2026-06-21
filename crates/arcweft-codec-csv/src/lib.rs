#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use arcweft_data::{
    Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape,
    Value,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CsvCodec;

impl Codec for CsvCodec {
    fn id(&self) -> FormatId {
        FormatId::new("csv")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["text/csv"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["csv"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let rows = value.as_seq()?;
        let headers = collect_headers(rows)?;
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record(&headers)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        rows.iter().enumerate().try_for_each(|(index, row)| {
            let record = row.as_record().map_err(|err| err.at_index(index))?;
            let values = headers.iter().map(|header| {
                record
                    .get(header)
                    .and_then(Value::stringify_scalar)
                    .unwrap_or_default()
            });
            writer.write_record(values).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string()).at_index(index)
            })
        })?;
        writer
            .into_inner()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut reader = csv::Reader::from_reader(input);
        let headers = reader
            .headers()
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let rows = reader
            .records()
            .enumerate()
            .map(|(row_index, record)| {
                let record = record.map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                        .at_index(row_index)
                })?;
                headers
                    .iter()
                    .zip(record.iter())
                    .map(|(header, value)| (header.clone(), Value::String(value.to_owned())))
                    .collect::<BTreeMap<_, _>>()
                    .pipe(Value::Record)
                    .pipe(Ok)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = Value::Seq(rows);
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn collect_headers(rows: &[Value]) -> Result<Vec<String>> {
    let Some(first) = rows.first() else {
        return Ok(Vec::new());
    };
    first
        .as_record()
        .map(|record| record.keys().cloned().collect())
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
