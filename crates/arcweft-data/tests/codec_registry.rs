use arcweft_data::{
    Codec, CodecRegistry, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape,
    Value,
};

#[derive(Clone, Copy)]
struct StaticCodec {
    id: &'static str,
    media_types: &'static [&'static str],
    extensions: &'static [&'static str],
}

impl Codec for StaticCodec {
    fn id(&self) -> FormatId {
        FormatId::new(self.id)
    }

    fn media_types(&self) -> &'static [&'static str] {
        self.media_types
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        value
            .stringify_scalar()
            .map(String::into_bytes)
            .ok_or_else(|| arcweft_data::DataError::invalid_type("scalar", value.type_name()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        _options: &DecodeOptions,
    ) -> Result<Value> {
        String::from_utf8(input.to_vec())
            .map(Value::String)
            .map_err(|error| {
                arcweft_data::DataError::new(
                    arcweft_data::DataErrorKind::InvalidEncoding,
                    error.to_string(),
                )
            })
    }
}

fn codec(
    id: &'static str,
    media_types: &'static [&'static str],
    extensions: &'static [&'static str],
) -> StaticCodec {
    StaticCodec {
        id,
        media_types,
        extensions,
    }
}

#[test]
fn registry_rejects_duplicate_codec_ids() {
    let mut registry = CodecRegistry::new();
    registry
        .register(codec("json", &["application/json"], &["json"]))
        .expect("first codec registers");

    let error = registry
        .register(codec("json", &["application/x-json-alt"], &["json2"]))
        .expect_err("duplicate id");

    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}

#[test]
fn registry_rejects_duplicate_media_types_across_codecs() {
    let mut registry = CodecRegistry::new();
    registry
        .register(codec("json", &["application/json"], &["json"]))
        .expect("first codec registers");

    let error = registry
        .register(codec(
            "json-alt",
            &["application/json; charset=utf-8"],
            &["json2"],
        ))
        .expect_err("duplicate media type");

    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}

#[test]
fn registry_rejects_duplicate_extensions_across_codecs() {
    let mut registry = CodecRegistry::new();
    registry
        .register(codec("yaml", &["application/yaml"], &["yaml"]))
        .expect("first codec registers");

    let error = registry
        .register(codec("yaml-alt", &["application/x-yaml"], &[".YAML"]))
        .expect_err("duplicate extension");

    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}

#[test]
fn registry_rejects_duplicate_aliases_inside_one_codec() {
    let mut registry = CodecRegistry::new();

    let error = registry
        .register(codec(
            "json",
            &["application/json", "application/json; charset=utf-8"],
            &["json"],
        ))
        .expect_err("duplicate media alias");

    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}

#[test]
fn registry_keeps_explicit_distinct_aliases() {
    let registry = CodecRegistry::new()
        .with(codec(
            "yaml",
            &["application/yaml", "application/x-yaml"],
            &["yaml", "yml"],
        ))
        .expect("distinct aliases register");

    assert_eq!(registry.by_id("yaml").expect("id").id().as_str(), "yaml");
    assert_eq!(
        registry
            .by_media_type("application/x-yaml; charset=utf-8")
            .expect("media")
            .id()
            .as_str(),
        "yaml"
    );
}
