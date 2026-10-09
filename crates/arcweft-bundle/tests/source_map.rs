use std::sync::Arc;

use arcweft_bundle::resource_codec::{
    FieldId, MAX_SOURCE_BYTES_PER_DOCUMENT, MAX_SOURCE_DISPLAY_NAME_BYTES,
    MAX_SOURCE_MAP_DOCUMENTS, MAX_SOURCE_MAP_TOTAL_UTF8_BYTES, ProductResourceEnvelope,
    ProductSectionCodecKind, ResourceField, ResourceWireType, SectionCodecBudget,
    SectionCodecError, SourceMapBuildError, SourceMapCodecError, SourceMapDocument,
    SourceMapDocumentInput, SourceMapSection, StringId, StringTable,
};
use arcweft_manifest_model::NormalizedProjectPath;
use arcweft_source::{
    MAX_PRODUCT_SOURCE_ID_INPUT_BYTES, SourceDocument, SourceDocumentId, SourceName,
};

const FIELD_SOURCE_MAP_TRANSCRIPT: FieldId = FieldId(1);
const SET_REVISION_OFFSET: usize = 4;
const PRIMARY_DOCUMENT_REF_OFFSET: usize = 36;
const FIRST_PRODUCT_REF_OFFSET: usize = 44;
const FIRST_REVISION_OFFSET: usize = 57;
const FIRST_EXTENT_OFFSET: usize = 89;
const FIRST_UTF8_LENGTH_OFFSET: usize = 97;
const FIRST_UTF8_OFFSET: usize = 105;

#[test]
fn source_map_primary_document_is_independent_of_canonical_document_order_and_round_trips() {
    let first = document(
        "project://first.arcw",
        SourceName::path("src/first.arcw"),
        "α",
    );
    let second = document("project://second.arcw", SourceName::Generated, "second");

    let ordered = SourceMapSection::try_from_documents(&[&first, &second]).expect("source map");
    let canonical_first = ordered
        .documents()
        .next()
        .expect("two-document source map")
        .document_id()
        .clone();
    let (primary, other) = if canonical_first == *first.identity().id() {
        (&second, &first)
    } else {
        (&first, &second)
    };
    let primary_after_canonical_sort =
        SourceMapSection::try_from_documents(&[primary, other]).expect("source map");

    assert_eq!(
        ordered
            .documents()
            .map(SourceMapDocument::document_id)
            .collect::<Vec<_>>(),
        primary_after_canonical_sort
            .documents()
            .map(SourceMapDocument::document_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ordered.source_set_revision(),
        primary_after_canonical_sort.source_set_revision()
    );
    assert_eq!(
        primary_after_canonical_sort
            .primary_document_id()
            .expect("non-empty source map has a primary document"),
        primary.identity().id()
    );
    assert_ne!(
        primary_after_canonical_sort
            .documents()
            .next()
            .expect("two-document source map")
            .document_id(),
        primary_after_canonical_sort
            .primary_document_id()
            .expect("non-empty source map has a primary document")
    );

    let bytes = primary_after_canonical_sort
        .encode_canonical_section()
        .expect("source map encodes");
    let decoded = SourceMapSection::decode_canonical_section(&bytes).expect("source map decodes");
    assert_eq!(decoded, primary_after_canonical_sort);
    assert_eq!(
        decoded
            .encode_canonical_section()
            .expect("decoded source map re-encodes"),
        bytes
    );
}

#[test]
fn source_map_rejects_missing_absent_and_out_of_bounds_primary_documents() {
    let source = document(
        "project://main.arcw",
        SourceName::path("src/main.arcw"),
        "main",
    );
    let bytes = SourceMapSection::try_from_documents(&[&source])
        .expect("source map")
        .encode_canonical_section()
        .expect("source map encodes");

    let missing = mutate_transcript(&bytes, |payload| {
        payload[PRIMARY_DOCUMENT_REF_OFFSET..PRIMARY_DOCUMENT_REF_OFFSET + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
    });
    assert_eq!(
        SourceMapSection::decode_canonical_section(&missing)
            .expect_err("a non-empty map requires a primary document"),
        SourceMapCodecError::MissingPrimaryDocument
    );

    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::SourceMap,
        SectionCodecBudget::default(),
    )
    .expect("source-map envelope");
    let absent_ref = envelope
        .strings
        .id_for("src/main.arcw")
        .expect("display path is in the string table")
        .0;
    let absent = mutate_transcript(&bytes, |payload| {
        payload[PRIMARY_DOCUMENT_REF_OFFSET..PRIMARY_DOCUMENT_REF_OFFSET + 4]
            .copy_from_slice(&absent_ref.to_le_bytes());
    });
    assert_eq!(
        SourceMapSection::decode_canonical_section(&absent)
            .expect_err("primary ID outside the inventory rejects"),
        SourceMapCodecError::PrimaryDocumentMissing(
            SourceDocumentId::try_new("src/main.arcw").expect("valid absent source ID")
        )
    );

    let out_of_bounds = mutate_transcript(&bytes, |payload| {
        payload[PRIMARY_DOCUMENT_REF_OFFSET..PRIMARY_DOCUMENT_REF_OFFSET + 4]
            .copy_from_slice(&u32::MAX.saturating_sub(1).to_le_bytes());
    });
    assert_eq!(
        SourceMapSection::decode_canonical_section(&out_of_bounds)
            .expect_err("out-of-bounds primary string reference rejects"),
        SourceMapCodecError::Envelope(SectionCodecError::StringOutOfBounds(StringId(u32::MAX - 1)))
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
        payload[..4].copy_from_slice(&2_u32.to_le_bytes());
    });
    assert!(matches!(
        SourceMapSection::decode_canonical_section(&schema),
        Err(SourceMapCodecError::UnsupportedSchema {
            actual: 2,
            expected: 1
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
    let exact_section =
        SourceMapSection::try_from_documents(&[&exact_bytes]).expect("exact document-byte limit");
    let encoded = exact_section
        .encode_canonical_section()
        .expect("exact limit encodes");
    let decoded = SourceMapSection::decode_canonical_section(&encoded)
        .expect("the decoder accepts the exact document-byte limit");
    assert_eq!(decoded, exact_section);
    assert_eq!(
        decoded.primary_document().unwrap().source_identity(),
        exact_bytes.identity()
    );
    drop(decoded);
    drop(exact_section);

    let extended = SourceMapSection::try_from_documents(&[])
        .unwrap()
        .try_with_document(&exact_bytes)
        .expect("extension accepts the exact document-byte limit");
    assert_eq!(
        extended.primary_document().unwrap().source_identity(),
        exact_bytes.identity()
    );
    let over_text = format!("{exact_text}x");
    let over_bytes = document("over.arcw", SourceName::Memory, &over_text);
    let expected_error = SourceMapBuildError::DocumentTooLarge {
        id: over_bytes.identity().id().clone(),
        bytes: MAX_SOURCE_BYTES_PER_DOCUMENT + 1,
        limit: MAX_SOURCE_BYTES_PER_DOCUMENT,
    };
    assert_eq!(
        SourceMapSection::try_from_documents(&[&over_bytes]).unwrap_err(),
        expected_error
    );
    assert_eq!(
        extended.try_with_document(&over_bytes).unwrap_err(),
        expected_error
    );
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
    let expected_revision = exact.source_set_revision();
    let expected_primary = exact.primary_document_id().cloned();
    let exact = exact
        .try_with_document(&documents[0])
        .expect("an exact existing source stays idempotent at the total-byte limit");
    assert_eq!(exact.source_set_revision(), expected_revision);
    assert_eq!(exact.primary_document_id(), expected_primary.as_ref());
    assert_eq!(
        exact.primary_document().unwrap().source_identity(),
        documents[0].identity()
    );

    let one = shared_document("over.arcw", Arc::<str>::from("x"));
    let expected_error = SourceMapBuildError::TotalBytesExceeded {
        actual: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES + 1,
        limit: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES,
    };
    assert_eq!(exact.try_with_document(&one).unwrap_err(), expected_error);
    let mut over = references;
    over.push(&one);
    assert_eq!(
        SourceMapSection::try_from_documents(&over).unwrap_err(),
        expected_error
    );
}

#[test]
fn source_map_document_count_limit_is_exact() {
    let exact = (0..MAX_SOURCE_MAP_DOCUMENTS)
        .map(|index| document(&format!("{index}.arcw"), SourceName::Memory, ""))
        .collect::<Vec<_>>();
    let references = exact.iter().collect::<Vec<_>>();
    let section = SourceMapSection::try_from_documents(&references).expect("exact count limit");
    assert_eq!(section.documents().len(), MAX_SOURCE_MAP_DOCUMENTS);
    let expected_revision = section.source_set_revision();
    let expected_primary = section.primary_document_id().cloned();
    let section = section
        .try_with_document(&exact[0])
        .expect("an exact existing source stays idempotent at the count limit");
    assert_eq!(section.source_set_revision(), expected_revision);
    assert_eq!(section.primary_document_id(), expected_primary.as_ref());

    let changed = document(
        exact[0].identity().id().as_str(),
        SourceName::Memory,
        "changed",
    );
    assert_eq!(
        section.clone().try_with_document(&changed).unwrap_err(),
        SourceMapBuildError::DuplicateDocument(exact[0].identity().id().clone())
    );

    let one_over = document("over.arcw", SourceName::path("/private/over.arcw"), "");
    let expected_error = SourceMapBuildError::TooManyDocuments {
        actual: MAX_SOURCE_MAP_DOCUMENTS + 1,
        limit: MAX_SOURCE_MAP_DOCUMENTS,
    };
    assert_eq!(
        section.try_with_document(&one_over).unwrap_err(),
        expected_error
    );
    let mut over = references;
    over.push(&one_over);
    assert_eq!(
        SourceMapSection::try_from_documents(&over).unwrap_err(),
        expected_error
    );
}

#[test]
fn source_map_count_limit_precedes_invalid_metadata_and_duplicate_identities() {
    let source = document("count.arcw", SourceName::path("/private/count.arcw"), "");
    let references = vec![&source; MAX_SOURCE_MAP_DOCUMENTS + 1];
    let expected_error = SourceMapBuildError::TooManyDocuments {
        actual: references.len(),
        limit: MAX_SOURCE_MAP_DOCUMENTS,
    };
    SourceMapSection::check_document_count(MAX_SOURCE_MAP_DOCUMENTS)
        .expect("a producer can preflight the exact count limit");
    assert_eq!(
        SourceMapSection::check_document_count(references.len()).unwrap_err(),
        expected_error
    );
    assert_eq!(
        SourceMapSection::try_from_documents(&references).unwrap_err(),
        expected_error
    );
    let inputs = vec![SourceMapDocumentInput::for_document(&source); references.len()];
    assert_eq!(
        SourceMapSection::try_from_inputs(&inputs).unwrap_err(),
        expected_error
    );
}

#[test]
fn source_map_codec_rejects_oversized_declared_text_before_payload_materialization() {
    let source = document("budget.arcw", SourceName::Memory, "");
    let encoded = SourceMapSection::try_from_documents(&[&source])
        .unwrap()
        .encode_canonical_section()
        .unwrap();
    for utf8_bytes in [MAX_SOURCE_BYTES_PER_DOCUMENT + 1, u64::MAX] {
        let oversized = mutate_transcript(&encoded, |payload| {
            payload[FIRST_UTF8_LENGTH_OFFSET..FIRST_UTF8_OFFSET]
                .copy_from_slice(&utf8_bytes.to_le_bytes());
        });
        assert_eq!(
            SourceMapSection::decode_canonical_section(&oversized).unwrap_err(),
            SourceMapCodecError::Build(SourceMapBuildError::DocumentTooLarge {
                id: source.identity().id().clone(),
                bytes: utf8_bytes,
                limit: MAX_SOURCE_BYTES_PER_DOCUMENT,
            }),
            "an inadmissible declared extent must reject before reading its absent payload"
        );
    }
}

#[test]
fn source_map_codec_charges_the_combined_text_budget_before_the_next_payload() {
    let chunk = Arc::<str>::from("x".repeat(document_byte_limit()));
    let chunk_document = shared_document("chunk.arcw", Arc::clone(&chunk));
    let documents = (0..9)
        .map(|index| document(&format!("budget-{index}.arcw"), SourceName::Memory, ""))
        .collect::<Vec<_>>();
    let references = documents.iter().collect::<Vec<_>>();
    let encoded = SourceMapSection::try_from_documents(&references)
        .unwrap()
        .encode_canonical_section()
        .unwrap();
    let oversized = mutate_transcript(&encoded, |payload| {
        // Each original record has no UTF-8 body. Keep its exact product/document
        // references, then append eight individually valid 8 MiB documents. The
        // ninth record claims one more byte, which is deliberately absent: the
        // source-map aggregate quota must reject before reading that next body.
        let records = payload[FIRST_PRODUCT_REF_OFFSET..].to_vec();
        let record_bytes = FIRST_UTF8_OFFSET - FIRST_PRODUCT_REF_OFFSET;
        assert_eq!(records.len(), documents.len() * record_bytes);
        payload.truncate(FIRST_PRODUCT_REF_OFFSET);
        payload.reserve_exact(
            usize::try_from(MAX_SOURCE_MAP_TOTAL_UTF8_BYTES).unwrap() + records.len(),
        );
        for (index, record) in records.chunks_exact(record_bytes).enumerate() {
            let start = payload.len();
            payload.extend_from_slice(record);
            let utf8_bytes = if index < 8 {
                MAX_SOURCE_BYTES_PER_DOCUMENT
            } else {
                1
            };
            let extent = start + FIRST_EXTENT_OFFSET - FIRST_PRODUCT_REF_OFFSET;
            let length = start + FIRST_UTF8_LENGTH_OFFSET - FIRST_PRODUCT_REF_OFFSET;
            payload[extent..extent + 8].copy_from_slice(&utf8_bytes.to_le_bytes());
            payload[length..length + 8].copy_from_slice(&utf8_bytes.to_le_bytes());
            if index < 8 {
                let revision = start + FIRST_REVISION_OFFSET - FIRST_PRODUCT_REF_OFFSET;
                payload[revision..revision + 32]
                    .copy_from_slice(chunk_document.identity().revision().as_bytes());
                payload.extend_from_slice(chunk.as_bytes());
            }
        }
    });
    assert_eq!(
        SourceMapSection::decode_canonical_section(&oversized).unwrap_err(),
        SourceMapCodecError::Build(SourceMapBuildError::TotalBytesExceeded {
            actual: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES + 1,
            limit: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES,
        })
    );
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

#[test]
fn projected_source_map_names_preserve_identity_and_are_relocation_deterministic() {
    let first = document(
        "arcweft-project://portable/src/序幕.arcw",
        SourceName::path("C:/first-checkout/src/序幕.arcw"),
        "α\n",
    );
    let second = document(
        "arcweft-project://portable/src/序幕.arcw",
        SourceName::path("D:/second-checkout/src/序幕.arcw"),
        "α\n",
    );
    let path = NormalizedProjectPath::new("src/序幕.arcw").unwrap();
    let first_map = SourceMapSection::try_from_inputs(&[SourceMapDocumentInput::for_project_file(
        &first, &path,
    )])
    .unwrap();
    let second_map =
        SourceMapSection::try_from_inputs(&[SourceMapDocumentInput::for_project_file(
            &second, &path,
        )])
        .unwrap();
    let primary = first_map.primary_document().unwrap();
    assert_eq!(primary.source_identity(), first.identity());
    assert_eq!(primary.text(), first.text());
    assert_eq!(primary.display_name(), &SourceName::path("src/序幕.arcw"));
    assert_eq!(
        first.display_name(),
        &SourceName::path("C:/first-checkout/src/序幕.arcw")
    );
    let bytes = first_map.encode_canonical_section().unwrap();
    assert_eq!(bytes, second_map.encode_canonical_section().unwrap());
    assert!(
        !bytes
            .windows(b"first-checkout".len())
            .any(|value| value == b"first-checkout")
    );
    let decoded = SourceMapSection::decode_canonical_section(&bytes).unwrap();
    assert_eq!(decoded, first_map);
    assert_eq!(decoded.encode_canonical_section().unwrap(), bytes);
}

#[test]
fn source_map_rejects_nonportable_display_paths_at_construction() {
    for path in [
        "C:/private/main.arcw",
        "/private/main.arcw",
        "../main.arcw",
        "src/../main.arcw",
        "src\\main.arcw",
        "src//main.arcw",
        ".",
    ] {
        let source = document(
            "arcweft-project://reject/main.arcw",
            SourceName::path(path),
            "main",
        );
        let error = SourceMapSection::try_from_documents(&[&source]).unwrap_err();
        assert!(
            matches!(error, SourceMapBuildError::InvalidDisplayPath { id, source: reason }
            if id == *source.identity().id() && reason == NormalizedProjectPath::new(path).unwrap_err()),
            "{path}"
        );
    }
}

#[test]
fn source_map_codec_rejects_absolute_display_metadata_before_publication() {
    let source = document(
        "project://main.arcw",
        SourceName::path("src/main.arcw"),
        "main",
    );
    let bytes = SourceMapSection::try_from_documents(&[&source])
        .unwrap()
        .encode_canonical_section()
        .unwrap();
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::SourceMap,
        SectionCodecBudget::default(),
    )
    .unwrap();
    let absolute_path = "z:/private/main.arcw";
    let strings = StringTable::new(envelope.strings.values().iter().map(|value| {
        if value == "src/main.arcw" {
            absolute_path.to_owned()
        } else {
            value.clone()
        }
    }))
    .unwrap();
    assert_eq!(
        envelope.strings.id_for("src/main.arcw"),
        strings.id_for(absolute_path),
        "the typed transcript retains its exact display string coordinate"
    );
    let tampered = ProductResourceEnvelope::new(
        envelope.header.codec,
        strings,
        envelope.public_ids,
        envelope.enums,
        envelope.fields,
        envelope.header.record_count,
    )
    .unwrap()
    .encode_canonical()
    .unwrap();
    assert!(
        matches!(SourceMapSection::decode_canonical_section(&tampered), Err(SourceMapCodecError::Build(SourceMapBuildError::InvalidDisplayPath { id, .. })) if id == *source.identity().id())
    );
}

#[test]
fn source_map_identity_projection_preserves_names_and_rejects_missing_stale_or_duplicate_sources() {
    let root = document(
        "project://root.arcw",
        SourceName::path("C:/local/root.arcw"),
        "root",
    );
    let child = document(
        "project://child.arcw",
        SourceName::path("C:/local/child.arcw"),
        "child",
    );
    let root_path = NormalizedProjectPath::new("src/main.arcw").unwrap();
    let child_path = NormalizedProjectPath::new("src/child.arcw").unwrap();
    let authored = SourceMapSection::try_from_inputs(&[
        SourceMapDocumentInput::for_project_file(&root, &root_path),
        SourceMapDocumentInput::for_project_file(&child, &child_path),
    ])
    .unwrap();
    let generated = document("engine://standard", SourceName::Generated, "standard");
    let full = authored.clone().try_with_document(&generated).unwrap();
    let selected = full
        .try_for_source_identities(&[root.identity(), child.identity()])
        .unwrap();
    assert_eq!(selected, authored);
    assert_eq!(full.documents().len(), 3);
    assert_eq!(selected.primary_document_id(), Some(root.identity().id()));
    let swapped = full
        .try_for_source_identities(&[child.identity(), root.identity()])
        .unwrap();
    assert_eq!(swapped.primary_document_id(), Some(child.identity().id()));
    assert_eq!(
        selected.source_set_revision(),
        swapped.source_set_revision()
    );
    for original in authored.documents() {
        assert_eq!(
            selected.get(original.id()).unwrap().product_source_ref(),
            original.product_source_ref()
        );
    }
    let missing = document("project://absent.arcw", SourceName::Memory, "");
    assert_eq!(
        full.try_for_source_identities(&[missing.identity()])
            .unwrap_err(),
        SourceMapBuildError::MissingDocument(missing.identity().id().clone())
    );
    let stale = document(
        root.identity().id().as_str(),
        SourceName::Memory,
        "changed root",
    );
    assert_eq!(
        full.try_for_source_identities(&[stale.identity()])
            .unwrap_err(),
        SourceMapBuildError::DocumentIdentityMismatch {
            expected: Box::new(stale.identity().clone()),
            actual: Box::new(root.identity().clone())
        }
    );
    assert_eq!(
        full.try_for_source_identities(&[root.identity(), root.identity()])
            .unwrap_err(),
        SourceMapBuildError::DuplicateDocument(root.identity().id().clone())
    );
}

#[test]
fn source_map_extension_keeps_project_metadata_and_generated_identity() {
    let source = document(
        "project://main.arcw",
        SourceName::path("C:/private/main.arcw"),
        "main",
    );
    let path = NormalizedProjectPath::new("main.arcw").unwrap();
    let map = SourceMapSection::try_from_inputs(&[SourceMapDocumentInput::for_project_file(
        &source, &path,
    )])
    .unwrap();
    assert_eq!(
        map.clone().try_with_document(&source).unwrap(),
        map,
        "the original absolute diagnostic name cannot replace admitted portable metadata"
    );
    let changed = document(
        source.identity().id().as_str(),
        source.display_name().clone(),
        "changed main",
    );
    assert_eq!(
        map.clone().try_with_document(&changed).unwrap_err(),
        SourceMapBuildError::DuplicateDocument(source.identity().id().clone())
    );
    let generated = document("engine://standard", SourceName::Generated, "generated");
    let extended = map.clone().try_with_document(&generated).unwrap();
    assert_eq!(extended.primary_document_id(), map.primary_document_id());
    assert_eq!(extended.primary_document(), map.primary_document());
    assert_eq!(
        extended.clone().try_with_document(&generated).unwrap(),
        extended
    );
    assert_eq!(
        extended
            .try_for_source_identities(&[source.identity()])
            .unwrap(),
        map
    );
    let bytes = extended.encode_canonical_section().unwrap();
    assert_eq!(
        SourceMapSection::decode_canonical_section(&bytes).unwrap(),
        extended
    );
}
