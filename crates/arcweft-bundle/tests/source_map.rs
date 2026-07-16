use std::sync::Arc;

use arcweft_bundle::resource_codec::{
    FieldId, MAX_PRODUCT_SOURCE_ID_INPUT_BYTES, MAX_SOURCE_BYTES_PER_DOCUMENT,
    MAX_SOURCE_DISPLAY_NAME_BYTES, MAX_SOURCE_MAP_DOCUMENTS, MAX_SOURCE_MAP_TOTAL_UTF8_BYTES,
    ProductResourceEnvelope, ProductSectionCodecKind, ResourceField, ResourceWireType,
    SectionCodecBudget, SourceMapBuildError, SourceMapCodecError, SourceMapSection,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

const FIELD_SOURCE_MAP_TRANSCRIPT: FieldId = FieldId(1);
const SET_REVISION_OFFSET: usize = 4;
const FIRST_PRODUCT_REF_OFFSET: usize = 40;
const FIRST_REVISION_OFFSET: usize = 53;
const FIRST_EXTENT_OFFSET: usize = 85;
const FIRST_UTF8_OFFSET: usize = 101;

#[test]
fn source_map_is_order_independent_and_round_trips_exact_bytes() {
    let first = document(
        "project://first.arcw",
        SourceName::path("src/first.arcw"),
        "α",
    );
    let second = document("project://second.arcw", SourceName::Generated, "second");

    let ordered = SourceMapSection::try_from_documents(&[&first, &second]).expect("source map");
    let reversed =
        SourceMapSection::try_from_documents(&[&second, &first]).expect("reversed source map");
    let bytes = ordered
        .encode_canonical_section()
        .expect("source map encodes");

    assert_eq!(
        bytes,
        reversed
            .encode_canonical_section()
            .expect("reversed source map encodes")
    );
    let decoded = SourceMapSection::decode_canonical_section(&bytes).expect("source map decodes");
    assert_eq!(decoded, ordered);
    assert_eq!(
        decoded
            .encode_canonical_section()
            .expect("decoded source map re-encodes"),
        bytes
    );
}

#[test]
fn source_map_rejects_duplicate_logical_documents() {
    let first = document("project://main.arcw", SourceName::path("main.arcw"), "same");
    let duplicate = document("project://main.arcw", SourceName::Memory, "same");

    assert!(matches!(
        SourceMapSection::try_from_documents(&[&first, &duplicate]),
        Err(SourceMapBuildError::DuplicateDocument(id))
            if id.as_str() == "project://main.arcw"
    ));
}

#[test]
fn source_map_rejects_digest_extent_set_revision_and_schema_tampering() {
    let source = document("main.arcw", SourceName::path("main.arcw"), "é");
    let bytes = SourceMapSection::try_from_documents(&[&source])
        .expect("source map")
        .encode_canonical_section()
        .expect("source map encodes");

    let digest = mutate_transcript(&bytes, |payload| payload[FIRST_REVISION_OFFSET] ^= 1);
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&digest),
        Err(SourceMapCodecError::RevisionMismatch { .. })
    ));

    let extent = mutate_transcript(&bytes, |payload| {
        payload[FIRST_EXTENT_OFFSET..FIRST_EXTENT_OFFSET + 8].copy_from_slice(&3_u64.to_le_bytes());
    });
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&extent),
        Err(SourceMapCodecError::ExtentMismatch { .. })
    ));

    let set_revision = mutate_transcript(&bytes, |payload| payload[SET_REVISION_OFFSET] ^= 1);
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&set_revision),
        Err(SourceMapCodecError::SourceSetRevisionMismatch)
    ));

    let schema = mutate_transcript(&bytes, |payload| {
        payload[..4].copy_from_slice(&1_u32.to_le_bytes());
    });
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&schema),
        Err(SourceMapCodecError::UnsupportedSchema {
            actual: 1,
            expected: 2
        })
    ));

    let invalid_utf8 = mutate_transcript(&bytes, |payload| payload[FIRST_UTF8_OFFSET] = 0xff);
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&invalid_utf8),
        Err(SourceMapCodecError::InvalidUtf8)
    ));
}

#[test]
fn source_map_rejects_product_id_mismatch_and_noncanonical_envelopes() {
    let first = document("first.arcw", SourceName::path("first.arcw"), "one");
    let second = document("second.arcw", SourceName::path("second.arcw"), "two");
    let bytes = SourceMapSection::try_from_documents(&[&first, &second])
        .expect("source map")
        .encode_canonical_section()
        .expect("source map encodes");

    let wrong_product = mutate_transcript(&bytes, |payload| {
        let current = u32::from_le_bytes(
            payload[FIRST_PRODUCT_REF_OFFSET..FIRST_PRODUCT_REF_OFFSET + 4]
                .try_into()
                .expect("product ref bytes"),
        );
        let other = u32::from(current == 0);
        payload[FIRST_PRODUCT_REF_OFFSET..FIRST_PRODUCT_REF_OFFSET + 4]
            .copy_from_slice(&other.to_le_bytes());
    });
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&wrong_product),
        Err(SourceMapCodecError::ProductSourceIdMismatch { .. })
    ));

    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::SourceMap,
        SectionCodecBudget::default(),
    )
    .expect("source-map envelope");
    let mut fields = envelope.fields.clone();
    fields.push(ResourceField::optional(
        FieldId(30_000),
        ResourceWireType::Bytes,
        b"future",
    ));
    let noncanonical = ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope encodes");
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&noncanonical),
        Err(SourceMapCodecError::NonCanonicalEncoding)
    ));

    assert!(
        SourceMapSection::decode_canonical_section(
            br#"{"source":{"label":"main.arcw","text":"old"}}"#
        )
        .is_err()
    );
}

#[test]
fn source_map_id_display_and_document_byte_limits_are_exact() {
    let exact_id = "i".repeat(MAX_PRODUCT_SOURCE_ID_INPUT_BYTES);
    let exact = document(&exact_id, SourceName::path("d"), "");
    SourceMapSection::try_from_documents(&[&exact]).expect("exact ID limit");
    let over_id = "i".repeat(MAX_PRODUCT_SOURCE_ID_INPUT_BYTES + 1);
    let over = document(&over_id, SourceName::path("d"), "");
    assert!(matches!(
        SourceMapSection::try_from_documents(&[&over]),
        Err(SourceMapBuildError::DocumentIdTooLong { .. })
    ));

    let exact_display = document(
        "display.arcw",
        SourceName::path("d".repeat(MAX_SOURCE_DISPLAY_NAME_BYTES)),
        "",
    );
    SourceMapSection::try_from_documents(&[&exact_display]).expect("exact display limit");
    let over_display = document(
        "display.arcw",
        SourceName::path("d".repeat(MAX_SOURCE_DISPLAY_NAME_BYTES + 1)),
        "",
    );
    assert!(matches!(
        SourceMapSection::try_from_documents(&[&over_display]),
        Err(SourceMapBuildError::DisplayNameTooLong { .. })
    ));

    let exact_text = "x".repeat(document_byte_limit());
    let exact_bytes = document("exact.arcw", SourceName::Memory, &exact_text);
    SourceMapSection::try_from_documents(&[&exact_bytes]).expect("exact document-byte limit");
    let over_text = format!("{exact_text}x");
    let over_bytes = document("over.arcw", SourceName::Memory, &over_text);
    assert!(matches!(
        SourceMapSection::try_from_documents(&[&over_bytes]),
        Err(SourceMapBuildError::DocumentTooLarge { .. })
    ));
}

#[test]
fn source_map_total_byte_limit_is_exact_and_candidate_first() {
    let chunk = Arc::<str>::from("x".repeat(document_byte_limit()));
    let documents = (0..8)
        .map(|index| shared_document(&format!("{index}.arcw"), Arc::clone(&chunk)))
        .collect::<Vec<_>>();
    let references = documents.iter().collect::<Vec<_>>();
    assert_eq!(
        documents
            .iter()
            .map(|document| document.identity().source_len())
            .sum::<u64>(),
        MAX_SOURCE_MAP_TOTAL_UTF8_BYTES
    );
    let exact = SourceMapSection::try_from_documents(&references).expect("exact total-byte limit");
    drop(exact);

    let one = shared_document("over.arcw", Arc::<str>::from("x"));
    let mut over = references;
    over.push(&one);
    assert!(matches!(
        SourceMapSection::try_from_documents(&over),
        Err(SourceMapBuildError::TotalBytesExceeded { .. })
    ));
}

#[test]
fn source_map_document_count_limit_is_exact() {
    let exact = (0..MAX_SOURCE_MAP_DOCUMENTS)
        .map(|index| document(&format!("{index}.arcw"), SourceName::Memory, ""))
        .collect::<Vec<_>>();
    let references = exact.iter().collect::<Vec<_>>();
    let section = SourceMapSection::try_from_documents(&references).expect("exact count limit");
    assert_eq!(section.documents().len(), MAX_SOURCE_MAP_DOCUMENTS);
    drop(section);

    let one_over = document("over.arcw", SourceName::Memory, "");
    let mut over = references;
    over.push(&one_over);
    assert!(matches!(
        SourceMapSection::try_from_documents(&over),
        Err(SourceMapBuildError::TooManyDocuments { .. })
    ));
}

fn document(id: &str, display_name: SourceName, text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("source document id"),
        display_name,
        Arc::<str>::from(text),
    )
    .expect("source document")
}

fn document_byte_limit() -> usize {
    usize::try_from(MAX_SOURCE_BYTES_PER_DOCUMENT)
        .expect("the source-document byte limit fits every supported test target")
}

fn shared_document(id: &str, text: Arc<str>) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("source document id"),
        SourceName::Memory,
        text,
    )
    .expect("source document")
}

fn mutate_transcript(bytes: &[u8], mutate: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let envelope = ProductResourceEnvelope::decode_all_fields(
        bytes,
        ProductSectionCodecKind::SourceMap,
        SectionCodecBudget::default(),
    )
    .expect("source-map envelope");
    let mut fields = envelope.fields.clone();
    let transcript = fields
        .iter_mut()
        .find(|field| field.id == FIELD_SOURCE_MAP_TRANSCRIPT)
        .expect("source-map transcript");
    mutate(&mut transcript.payload);
    ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope re-encodes")
}
