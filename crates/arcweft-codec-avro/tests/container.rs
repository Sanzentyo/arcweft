use std::{cell::Cell, process::Command};

use apache_avro::{Reader, types::Value as AvroValue};
use arcweft_codec_avro::container::{AvroDatum, DatumWriter, ObjectContainer};
use arcweft_data::{DataErrorKind, EncodeOptions, Result};

struct Long(i64);
impl AvroDatum for Long {
    fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
        writer.integer(self.0)
    }
}

fn longs() -> Vec<u8> {
    EncodeOptions::default()
        .encode_with(|output| {
            ObjectContainer::new("\"long\"")?.encode_to(|| [Long(3), Long(27)].into_iter(), output)
        })
        .unwrap()
}

#[test]
fn container_has_stable_metadata_order_and_standard_binary_framing() {
    let bytes = longs();
    let prefix = b"Obj\x01\x04\x14avro.codec\x08null\x16avro.schema\x0c\"long\"\0";
    assert!(bytes.starts_with(prefix));
    assert_eq!(&bytes[prefix.len() + 16..prefix.len() + 20], &[4, 4, 6, 54]);
    assert_eq!(
        &bytes[prefix.len()..prefix.len() + 16],
        &bytes[bytes.len() - 16..]
    );
    assert_eq!(
        Reader::new(bytes.as_slice())
            .unwrap()
            .collect::<apache_avro::AvroResult<Vec<_>>>()
            .unwrap(),
        [AvroValue::Long(3), AvroValue::Long(27)]
    );
    assert_eq!(bytes, longs());
    let changed = EncodeOptions::default()
        .encode_with(|output| {
            ObjectContainer::new("\"long\"")?.encode_to(|| [Long(3), Long(28)].into_iter(), output)
        })
        .unwrap();
    assert_ne!(
        &bytes[prefix.len()..prefix.len() + 16],
        &changed[prefix.len()..prefix.len() + 16]
    );
}

#[test]
fn schema_directed_writer_rejects_wrong_kind_missing_and_reordered_fields() {
    enum Invalid {
        WrongKind,
        Missing,
        Reordered,
    }
    impl AvroDatum for Invalid {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            writer.record(|record| match self {
                Self::WrongKind => record.field("first", |writer| writer.string("wrong")),
                Self::Missing => Ok(()),
                Self::Reordered => record.field("second", |writer| writer.boolean(false)),
            })
        }
    }
    let schema = ObjectContainer::new(r#"{"type":"record","name":"Order","fields":[{"name":"first","type":"boolean"},{"name":"second","type":"boolean"}]}"#).unwrap();
    for value in [Invalid::WrongKind, Invalid::Missing, Invalid::Reordered] {
        let error = EncodeOptions::default()
            .encode_with(|output| schema.encode_to(|| std::iter::once(&value), output))
            .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
    }
}

#[test]
fn deterministic_child_probe() {
    let bytes = longs();
    println!(
        "AVRO_BYTES:{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}

#[test]
fn container_bytes_match_across_independent_processes() {
    let expected = longs()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for _ in 0..3 {
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "deterministic_child_probe", "--nocapture"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8(result.stdout).unwrap();
        let actual = stdout
            .lines()
            .find_map(|line| line.strip_prefix("AVRO_BYTES:"))
            .expect("child bytes");
        assert_eq!(actual, expected);
    }
}

#[test]
fn container_admits_the_complete_length_before_copying_any_output() {
    struct Text<'a> {
        text: &'a str,
        visits: Cell<usize>,
    }
    impl AvroDatum for Text<'_> {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            self.visits.set(self.visits.get() + 1);
            writer.string(self.text)
        }
    }
    let container = ObjectContainer::new("\"string\"").unwrap();
    let text = "蟹".repeat(100_000);
    let datum = Text {
        text: &text,
        visits: Cell::new(0),
    };
    let full = EncodeOptions::default()
        .encode_with(|output| container.encode_to(|| std::iter::once(&datum), output))
        .unwrap();
    for maximum in [0, 128, full.len() - 1] {
        datum.visits.set(0);
        let error = EncodeOptions {
            max_output_len: maximum,
            ..EncodeOptions::default()
        }
        .encode_with(|output| {
            let result = container.encode_to(|| std::iter::once(&datum), output);
            assert_eq!(
                output.remaining_len().unwrap(),
                maximum,
                "measurement must not copy a header or payload"
            );
            result
        })
        .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert!(datum.visits.get() <= 1, "no emission pass on refusal");
    }
    let exact = EncodeOptions {
        max_output_len: full.len(),
        ..EncodeOptions::default()
    }
    .encode_with(|output| container.encode_to(|| std::iter::once(&datum), output))
    .unwrap();
    assert_eq!(exact, full);
}

#[test]
fn zero_byte_datums_preserve_large_array_and_record_counts() {
    struct Null;
    impl AvroDatum for Null {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            writer.null()
        }
    }
    struct Nulls;
    impl AvroDatum for Nulls {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            writer.array(std::iter::repeat_with(|| Null).take(100_000))
        }
    }
    let options = EncodeOptions {
        max_output_len: 256,
        ..EncodeOptions::default()
    };
    let bytes = options
        .encode_with(|output| {
            ObjectContainer::new(r#"{"type":"array","items":"null"}"#)?
                .encode_to(|| std::iter::once(Nulls), output)
        })
        .unwrap();
    let rows = Reader::new(bytes.as_slice())
        .unwrap()
        .collect::<apache_avro::AvroResult<Vec<_>>>()
        .unwrap();
    assert_eq!(rows, [AvroValue::Array(vec![AvroValue::Null; 100_000])]);
    let container = ObjectContainer::new("\"null\"").unwrap();
    let bytes = options
        .encode_with(|output| {
            container.encode_to(|| std::iter::repeat_with(|| Null).take(100_000), output)
        })
        .unwrap();
    assert_eq!(
        Reader::new(bytes.as_slice())
            .unwrap()
            .map(|value| value.unwrap())
            .filter(|value| *value == AvroValue::Null)
            .count(),
        100_000
    );
    let empty = options
        .encode_with(|output| container.encode_to(|| std::iter::empty::<Null>(), output))
        .unwrap();
    assert_eq!(Reader::new(empty.as_slice()).unwrap().count(), 0);
    assert_ne!(bytes, empty);
}

#[test]
fn changed_datums_and_json_strings_cannot_publish_invalid_bytes() {
    struct Changing(Cell<bool>);
    impl AvroDatum for Changing {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            let value = self.0.get();
            self.0.set(!value);
            writer.boolean(value)
        }
    }
    let changing = Changing(Cell::new(false));
    let error = EncodeOptions::default()
        .encode_with(|output| {
            ObjectContainer::new("\"boolean\"")?.encode_to(|| std::iter::once(&changing), output)
        })
        .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    struct ChangingJson(Cell<bool>);
    impl serde::Serialize for ChangingJson {
        fn serialize<S: serde::Serializer>(
            &self,
            serializer: S,
        ) -> std::result::Result<S::Ok, S::Error> {
            let value = self.0.get();
            self.0.set(!value);
            serializer.serialize_str(if value { "a" } else { "b" })
        }
    }
    impl AvroDatum for ChangingJson {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            writer.json_string(self)
        }
    }
    let changing_json = ChangingJson(Cell::new(false));
    let error = EncodeOptions::default()
        .encode_with(|output| {
            ObjectContainer::new("\"string\"")?
                .encode_to(|| std::iter::once(&changing_json), output)
        })
        .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn json_strings_stream_escaped_utf8_under_the_exact_container_limit() {
    struct Json<'a>(&'a str);
    impl AvroDatum for Json<'_> {
        fn write_avro(&self, writer: DatumWriter<'_>) -> Result<()> {
            writer.json_string(self.0)
        }
    }
    let container = ObjectContainer::new("\"string\"").unwrap();
    let text = "\0\"\\\n蟹".repeat(100);
    let bytes = EncodeOptions::default()
        .encode_with(|output| container.encode_to(|| std::iter::once(Json(&text)), output))
        .unwrap();
    for maximum in [0, 128, bytes.len() - 1, bytes.len()] {
        let result = EncodeOptions {
            max_output_len: maximum,
            ..EncodeOptions::default()
        }
        .encode_with(|output| container.encode_to(|| std::iter::once(Json(&text)), output));
        if maximum < bytes.len() {
            assert_eq!(result.unwrap_err().kind(), &DataErrorKind::LimitExceeded);
        } else {
            assert_eq!(result.unwrap(), bytes);
        }
    }
    let actual = Reader::new(bytes.as_slice())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        actual,
        AvroValue::String(serde_json::to_string(&text).unwrap())
    );
}
