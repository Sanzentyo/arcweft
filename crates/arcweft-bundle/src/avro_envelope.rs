//! The bundle-specific datum grammar; framing and bounded output belong to the
//! shared Avro container owner. JSON is streamed as the envelope's string field.

use arcweft_codec_avro::container::{AvroDatum, DatumWriter, ObjectContainer};
use arcweft_data::EncodeOptions;

use crate::{ARCWEFT_BUNDLE_SCHEMA_VERSION, ArcweftBundle, BundleCodecError, BundleFormat};

const SCHEMA: &str = r#"{
  "type": "record", "name": "ArcweftBundleEnvelope", "namespace": "org.arcweft.bundle",
  "fields": [
    {"name": "schema_version", "type": "int"},
    {"name": "payload_format", "type": "string"},
    {"name": "payload_json", "type": "string"}
  ]
}"#;

struct Envelope<'a>(&'a ArcweftBundle);

impl AvroDatum for Envelope<'_> {
    fn write_avro(&self, writer: DatumWriter<'_>) -> arcweft_data::Result<()> {
        writer.record(|record| {
            record.field("schema_version", |writer| {
                writer.integer(i64::from(ARCWEFT_BUNDLE_SCHEMA_VERSION))
            })?;
            record.field("payload_format", |writer| {
                writer.string(BundleFormat::Json.as_str())
            })?;
            record.field("payload_json", |writer| writer.json_string(self.0))
        })
    }
}

impl ArcweftBundle {
    pub(super) fn to_avro_envelope_bytes(&self) -> Result<Vec<u8>, BundleCodecError> {
        let encode = || {
            let container = ObjectContainer::new(SCHEMA)?;
            EncodeOptions::default().encode_with(|output| {
                container.encode_to(|| std::iter::once(Envelope(self)), output)
            })
        };
        encode().map_err(
            |error: arcweft_data::DataError| BundleCodecError::EncodeFormat {
                format: BundleFormat::Avro,
                message: error.to_string(),
            },
        )
    }
}
