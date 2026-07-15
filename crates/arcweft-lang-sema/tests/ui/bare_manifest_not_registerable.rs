use arcweft_character::{
    registration_catalog::SourceBackedCharacterCatalog,
    manifest::CharacterManifest,
};
use arcweft_source::SourceDocumentIdentity;

fn bypass(source: SourceDocumentIdentity, manifest: CharacterManifest) {
    let _ = SourceBackedCharacterCatalog::try_new(source, vec![manifest]);
}

fn main() {}
