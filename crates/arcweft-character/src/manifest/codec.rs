use super::{CharacterManifest, CharacterRuntimeDecodeError, registration};

impl CharacterManifest {
    /// Decodes a runtime-only manifest where source-token provenance is not consumed.
    ///
    /// Project registration must use
    /// [`registration::SourceBackedCharacterManifest::decode_registration_json`].
    pub fn decode_runtime_json(source: &str) -> Result<Self, CharacterRuntimeDecodeError> {
        registration::decode_runtime_json(source)
    }

    /// Serializes one validated manifest with deterministic pretty formatting.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut json| {
            json.push('\n');
            json
        })
    }
}
