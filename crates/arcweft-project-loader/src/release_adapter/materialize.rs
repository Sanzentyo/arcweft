use arcweft_bundle::release::archive::ExternalPayloadMaterializationMode;

pub fn materialization_mode_name(mode: ExternalPayloadMaterializationMode) -> &'static str {
    match mode {
        ExternalPayloadMaterializationMode::MetadataOnly => "metadata_only",
        ExternalPayloadMaterializationMode::RequiredResidency => "required_residency",
        ExternalPayloadMaterializationMode::AllPayloads => "all_payloads",
    }
}
