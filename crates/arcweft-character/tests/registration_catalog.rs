use arcweft_character::{
    manifest::registration::SourceBackedCharacterManifest,
    registration_catalog::{SourceBackedCharacterCatalog, SourceBackedCharacterCatalogError},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

#[test]
fn duplicate_owner_inside_catalog_is_error() {
    let source = include_str!("fixtures/zundamon.awchar/character.awchar.json");
    let first_document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-project://game/first.awchar.json").expect("first id"),
        SourceName::path("first.awchar.json"),
        source,
    )
    .expect("first document");
    let duplicate_document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-project://game/duplicate.awchar.json")
            .expect("duplicate id"),
        SourceName::path("duplicate.awchar.json"),
        source,
    )
    .expect("duplicate document");
    let first = SourceBackedCharacterManifest::decode_registration_json(&first_document)
        .expect("first manifest");
    let duplicate = SourceBackedCharacterManifest::decode_registration_json(&duplicate_document)
        .expect("duplicate manifest");

    let error = SourceBackedCharacterCatalog::try_new(
        first_document.identity().clone(),
        vec![first, duplicate],
    )
    .expect_err("duplicate owner");
    let SourceBackedCharacterCatalogError::DuplicateOwner {
        owner,
        first,
        duplicate,
    } = error
    else {
        panic!("expected duplicate owner");
    };
    assert_eq!(owner.as_str(), "character.zundamon");
    assert_eq!(
        first.source().id().as_str(),
        "arcweft-project://game/first.awchar.json"
    );
    assert_eq!(
        duplicate.source().id().as_str(),
        "arcweft-project://game/duplicate.awchar.json"
    );
    assert_eq!(&source[first.range().as_range()], "\"character.zundamon\"");
    assert_eq!(
        &source[duplicate.range().as_range()],
        "\"character.zundamon\""
    );
}
