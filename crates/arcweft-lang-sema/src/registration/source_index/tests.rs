use std::sync::Arc;

use arcweft_character::{
    id::CharacterId,
    manifest::registration::{CharacterManifestRootField, CharacterManifestTokenPath},
    registration_catalog::SourceBackedCharacterCatalog,
    symbol::CharacterSymbolDescriptor,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceRange, SourceSetRevisionError};

use crate::{
    env::TypeCheckEnv,
    registration::{ProjectRegistrationFacts, RegisteredSemanticWorld},
    test_support::character_project::{
        backed_manifest, character_binding_paths, declaration_span, external_fact,
        one_character_facts, register, root_project, sample_manifest, source_document,
    },
};

use super::{
    CharacterDeclarationSource, CharacterDefinitionIndex, CharacterDefinitionIndexBuildError,
    CharacterDefinitionIndexBuildReport, CharacterDefinitionIndexCode,
    CharacterDefinitionLimitKind, CharacterDefinitionLimits, CharacterDefinitionSpanError,
    IndexBuilder, validate_declaration_source,
};

fn registered_fixture(profile: &str) -> (ProjectRegistrationFacts, RegisteredSemanticWorld) {
    let (root, project, world) = root_project(profile);
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("the source-index fixture registers");
    (facts, registered)
}

fn manifest_document(facts: &ProjectRegistrationFacts) -> Arc<SourceDocument> {
    let identity = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest")
        .source_map()
        .document();
    facts
        .documents()
        .find(|document| document.identity() == identity)
        .cloned()
        .expect("fixture owns the manifest document")
}

fn owner_descriptor() -> CharacterSymbolDescriptor {
    CharacterSymbolDescriptor::Owner {
        character: CharacterId::try_new("character.akane").expect("fixture character"),
    }
}

fn root_token_path() -> CharacterManifestTokenPath {
    CharacterManifestTokenPath::Root(CharacterManifestRootField::Character)
}

fn report_contains(
    errors: &[CharacterDefinitionIndexBuildError],
    predicate: impl Fn(&CharacterDefinitionIndexBuildError) -> bool,
) {
    assert!(
        errors.iter().any(predicate),
        "expected error is absent from {errors:#?}"
    );
}

#[test]
fn missing_manifest_document_is_rejected() {
    let (facts, registered) = registered_fixture("source-index-missing-document");
    let (_missing_document, missing_manifest) = backed_manifest(
        "arcweft-project://registration-tests/characters/missing.awchar.json",
        &sample_manifest("layers/body.png"),
    );
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );

    builder.admit_manifest(&missing_manifest);

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::MissingDocument { identity }
                if identity == missing_manifest.source_map().document()
        )
    });
}

#[test]
fn conflicting_manifest_document_revision_is_rejected() {
    let (facts, registered) = registered_fixture("source-index-conflicting-document");
    let original = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest")
        .source_map()
        .document();
    let (_conflicting_document, conflicting_manifest) = backed_manifest(
        original.id().as_str(),
        &sample_manifest("layers/body-changed.png"),
    );
    assert_ne!(original, conflicting_manifest.source_map().document());
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );

    builder.admit_manifest(&conflicting_manifest);

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::ConflictingDocument {
                id,
                first,
                conflicting,
            } if id == original.id()
                && first == original
                && conflicting == conflicting_manifest.source_map().document()
        )
    });
}

#[test]
fn missing_admitted_document_is_a_typed_build_error() {
    let (facts, registered) = registered_fixture("source-index-missing-admitted-document");
    let manifest = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest");
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    builder.admit_manifest(manifest);
    assert!(builder.errors.is_empty());
    builder
        .documents
        .remove(manifest.source_map().document().id());

    builder.admit_descriptor(manifest, owner_descriptor());

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::MissingDocument { identity }
                if identity == manifest.source_map().document()
        )
    });
}

#[test]
fn declaration_span_from_another_source_is_rejected() {
    let expected = source_document(
        "arcweft-project://registration-tests/characters/expected.awchar.json",
        "\"character.akane\"",
    );
    let actual = source_document(
        "arcweft-project://registration-tests/characters/actual.awchar.json",
        "\"character.akane\"",
    );
    let source = CharacterDeclarationSource {
        token_path: root_token_path(),
        value_span: actual
            .span(SourceRange::new(0, actual.text().len()))
            .expect("value span"),
        selection_span: actual
            .span(SourceRange::new(1, actual.text().len() - 1))
            .expect("selection span"),
    };

    let error =
        validate_declaration_source(&owner_descriptor(), expected.identity(), &expected, &source)
            .expect_err("a foreign source identity must fail");

    assert!(matches!(
        *error,
        CharacterDefinitionIndexBuildError::SpanSourceMismatch {
            expected: ref expected_identity,
            actual: ref actual_identity,
            ..
        } if expected_identity == expected.identity() && actual_identity == actual.identity()
    ));
}

#[test]
fn selection_outside_value_is_rejected() {
    let document = source_document(
        "arcweft-project://registration-tests/characters/outside.awchar.json",
        "0123456789",
    );
    let source = CharacterDeclarationSource {
        token_path: root_token_path(),
        value_span: document.span(SourceRange::new(2, 7)).expect("value span"),
        selection_span: document
            .span(SourceRange::new(1, 6))
            .expect("selection span"),
    };

    let error =
        validate_declaration_source(&owner_descriptor(), document.identity(), &document, &source)
            .expect_err("selection outside its full value must fail");

    assert!(matches!(
        *error,
        CharacterDefinitionIndexBuildError::InvalidSpan {
            reason: CharacterDefinitionSpanError::SelectionOutsideValue,
            ..
        }
    ));
}

#[test]
fn selection_that_includes_quotes_is_rejected() {
    let document = source_document(
        "arcweft-project://registration-tests/characters/quoted.awchar.json",
        "0123456789",
    );
    let value = document.span(SourceRange::new(2, 7)).expect("value span");
    let source = CharacterDeclarationSource {
        token_path: root_token_path(),
        value_span: value.clone(),
        selection_span: value,
    };

    let error =
        validate_declaration_source(&owner_descriptor(), document.identity(), &document, &source)
            .expect_err("selection including quote bytes must fail");

    assert!(matches!(
        *error,
        CharacterDefinitionIndexBuildError::InvalidSpan {
            reason: CharacterDefinitionSpanError::SelectionIncludesQuote,
            ..
        }
    ));
}

#[test]
fn duplicate_source_fact_is_rejected_instead_of_deduplicated() {
    let (facts, registered) = registered_fixture("source-index-duplicate-fact");
    let manifest = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest");
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    builder.admit_manifest(manifest);
    assert!(builder.errors.is_empty());

    builder.admit_manifest(manifest);

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::DuplicateSourceFact { .. }
        )
    });
}

#[test]
fn inconsistent_source_fact_is_rejected() {
    let (facts, registered) = registered_fixture("source-index-inconsistent-fact");
    let manifest = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest");
    let document = manifest_document(&facts);
    let descriptor = owner_descriptor();
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    builder.admit_manifest(manifest);
    builder
        .declarations
        .get_mut(&descriptor)
        .expect("owner declaration")
        .first_mut()
        .expect("owner source")
        .selection_span = document
        .span(SourceRange::new(0, 1))
        .expect("altered source range");

    builder.admit_manifest(manifest);

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::InconsistentSourceFact {
                descriptor: actual,
                ..
            } if actual == &descriptor
        )
    });
}

#[test]
fn omitted_primary_descriptor_is_rejected() {
    let (facts, registered) = registered_fixture("source-index-missing-descriptor");
    let manifest = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest");
    let descriptor = owner_descriptor();
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    builder.admit_manifest(manifest);
    builder.declarations.remove(&descriptor);

    builder.audit_descriptor_inventory();

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::DescriptorSetMismatch {
                missing,
                unexpected,
            } if missing == std::slice::from_ref(&descriptor) && unexpected.is_empty()
        )
    });
}

#[test]
fn unexpected_primary_descriptor_is_rejected() {
    let (facts, registered) = registered_fixture("source-index-unexpected-descriptor");
    let manifest = facts
        .catalogs()
        .next()
        .expect("fixture catalog")
        .manifests()
        .next()
        .expect("fixture manifest");
    let unexpected = CharacterSymbolDescriptor::Owner {
        character: CharacterId::try_new("character.unexpected").expect("unexpected character"),
    };
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    builder.admit_manifest(manifest);
    let source = builder
        .declarations
        .get(&owner_descriptor())
        .expect("owner declaration")
        .first()
        .expect("owner source")
        .clone();
    builder
        .declarations
        .insert(unexpected.clone(), vec![source]);

    builder.audit_descriptor_inventory();

    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::DescriptorSetMismatch {
                missing,
                unexpected: actual,
            } if missing.is_empty() && actual == std::slice::from_ref(&unexpected)
        )
    });
}

#[test]
fn exact_reduced_index_limits_accept_the_fixture() {
    let (facts, registered) = registered_fixture("source-index-exact-limits");
    let source_bytes = manifest_document(&facts).identity().source_len();
    let limits = CharacterDefinitionLimits {
        indexed_manifests: 1,
        descriptors: 4,
        documents: 1,
        declaration_sources_per_descriptor: 1,
        source_bytes,
        build_work: 13,
        ..CharacterDefinitionLimits::PRODUCTION
    };

    let index = CharacterDefinitionIndex::try_build_with_limits(
        &facts,
        registered.symbols(),
        registered.environment(),
        limits,
    )
    .expect("the fixture exactly at every reduced bound builds");

    assert_eq!(index.manifest_count(), 1);
    assert_eq!(index.len(), 4);
    assert_eq!(index.documents().len(), 1);
}

#[test]
fn one_over_each_reduced_index_bound_fails_closed() {
    let (facts, registered) = registered_fixture("source-index-one-over-limits");
    let source_bytes = manifest_document(&facts).identity().source_len();
    let cases = [
        (
            CharacterDefinitionLimits {
                indexed_manifests: 0,
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::IndexedManifests,
        ),
        (
            CharacterDefinitionLimits {
                descriptors: 3,
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::Descriptors,
        ),
        (
            CharacterDefinitionLimits {
                documents: 0,
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::Documents,
        ),
        (
            CharacterDefinitionLimits {
                declaration_sources_per_descriptor: 0,
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
        ),
        (
            CharacterDefinitionLimits {
                source_bytes: source_bytes
                    .checked_sub(1)
                    .expect("fixture source is non-empty"),
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::SourceBytes,
        ),
        (
            CharacterDefinitionLimits {
                build_work: 12,
                ..CharacterDefinitionLimits::PRODUCTION
            },
            CharacterDefinitionLimitKind::BuildWork,
        ),
    ];

    for (limits, expected_kind) in cases {
        let report = CharacterDefinitionIndex::try_build_with_limits(
            &facts,
            registered.symbols(),
            registered.environment(),
            limits,
        )
        .expect_err("one-over input must not publish an index");
        report_contains(report.errors(), |error| {
            matches!(
                error,
                CharacterDefinitionIndexBuildError::Limit { kind, .. }
                    if *kind == expected_kind
            )
        });
    }
}

#[test]
fn diagnostic_report_is_bounded_and_deterministic() {
    let errors = vec![
        CharacterDefinitionIndexBuildError::Limit {
            kind: CharacterDefinitionLimitKind::Documents,
            observed: 2,
            maximum: 1,
        },
        CharacterDefinitionIndexBuildError::Limit {
            kind: CharacterDefinitionLimitKind::Descriptors,
            observed: 5,
            maximum: 4,
        },
        CharacterDefinitionIndexBuildError::Limit {
            kind: CharacterDefinitionLimitKind::SourceBytes,
            observed: 11,
            maximum: 10,
        },
    ];
    let exact = CharacterDefinitionIndexBuildReport::new(errors.clone(), 3);
    assert_eq!(exact.errors().len(), 3);
    assert_eq!(exact.omitted_errors(), 0);
    let zero = CharacterDefinitionIndexBuildReport::new(errors.clone(), 0);
    assert!(zero.errors().is_empty());
    assert_eq!(zero.omitted_errors(), 3);
    let huge = CharacterDefinitionIndexBuildReport::new(errors.clone(), u64::MAX);
    assert_eq!(huge, exact);
    let duplicate =
        CharacterDefinitionIndexBuildReport::new(vec![errors[0].clone(), errors[0].clone()], 1);
    assert_eq!(duplicate.errors(), &errors[..1]);
    assert_eq!(duplicate.omitted_errors(), 0);
    let visible_overflow = CharacterDefinitionIndexBuildReport::arithmetic_overflow(1);
    assert!(matches!(
        visible_overflow.errors(),
        [CharacterDefinitionIndexBuildError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::Diagnostics,
        }]
    ));
    assert_eq!(visible_overflow.omitted_errors(), 0);
    let bounded_overflow = CharacterDefinitionIndexBuildReport::arithmetic_overflow(0);
    assert!(bounded_overflow.errors().is_empty());
    assert_eq!(bounded_overflow.omitted_errors(), 1);

    let mut reversed = errors.clone();
    reversed.reverse();
    let first = CharacterDefinitionIndexBuildReport::new(errors, 2);
    let second = CharacterDefinitionIndexBuildReport::new(reversed, 2);

    assert_eq!(first, second);
    assert_eq!(first.errors().len(), 2);
    assert_eq!(first.omitted_errors(), 1);
}

#[test]
fn source_revision_and_counter_overflows_are_typed_and_fail_closed() {
    let (facts, registered) = registered_fixture("source-index-overflow");
    let mut builder = IndexBuilder::new(
        &facts,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    );
    source_revision_overflows_are_typed(&mut builder);
    conflicting_source_revisions_are_typed(&mut builder);
    counter_overflows_are_typed(&mut builder);
}

fn source_revision_overflows_are_typed(builder: &mut IndexBuilder<'_>) {
    builder.record_source_revision_error(SourceSetRevisionError::DocumentCountOverflow);
    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::Documents,
            }
        )
    });
    builder.record_source_revision_error(SourceSetRevisionError::DocumentIdLengthOverflow {
        id: SourceDocumentId::try_new("arcweft-project://source-index/oversized-id")
            .expect("oversized document id"),
        length: usize::MAX,
    });
    assert_eq!(
        builder
            .errors
            .iter()
            .filter(|error| matches!(
                error,
                CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Documents,
                }
            ))
            .count(),
        2
    );
}

fn conflicting_source_revisions_are_typed(builder: &mut IndexBuilder<'_>) {
    let first = source_document("arcweft-project://source-index/conflict", "first");
    let conflicting = source_document("arcweft-project://source-index/conflict", "second");
    builder.documents.clear();
    builder.documents.insert(
        SourceDocumentId::try_new("arcweft-project://source-index/key-a").expect("first map key"),
        Arc::clone(&first),
    );
    builder.documents.insert(
        SourceDocumentId::try_new("arcweft-project://source-index/key-b")
            .expect("conflicting map key"),
        Arc::clone(&conflicting),
    );
    assert!(builder.source_revision().is_none());
    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::ConflictingSourceRevision {
                id,
                first_revision,
                first_len,
                conflicting_revision,
                conflicting_len,
            } if id == first.identity().id()
                && *first_revision == first.identity().revision()
                && *first_len == first.identity().source_len()
                && *conflicting_revision == conflicting.identity().revision()
                && *conflicting_len == conflicting.identity().source_len()
        )
    });
    let conflict_error = builder
        .errors
        .iter()
        .find(|error| {
            matches!(
                error,
                CharacterDefinitionIndexBuildError::ConflictingSourceRevision { .. }
            )
        })
        .expect("source revision conflict is retained");
    assert_eq!(
        conflict_error.code(),
        CharacterDefinitionIndexCode::ConflictingDocument
    );
}

fn counter_overflows_are_typed(builder: &mut IndexBuilder<'_>) {
    assert!(!builder.charge_counter(
        CharacterDefinitionLimitKind::IndexedManifests,
        u64::MAX,
        1,
        u64::MAX,
    ));
    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::IndexedManifests,
            }
        )
    });

    builder.work = u64::MAX;
    builder.build_exhausted = false;
    assert!(!builder.charge_work(1));
    assert!(builder.build_exhausted);
    report_contains(&builder.errors, |error| {
        matches!(
            error,
            CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::BuildWork,
            }
        )
    });
}

#[test]
fn co_definition_order_is_deterministic_across_catalog_order() {
    let (root, project, world) = root_project("source-index-order");
    let manifest = sample_manifest("layers/body.png");
    let (first_document, first) = backed_manifest(
        "arcweft-project://registration-tests/characters/order-a.awchar.json",
        &manifest,
    );
    let (second_document, second) = backed_manifest(
        "arcweft-project://registration-tests/characters/order-b.awchar.json",
        &manifest,
    );
    let owner = manifest.character().clone();
    let declaration = declaration_span(&first);
    let fact = external_fact(
        owner.as_str(),
        &character_binding_paths(&owner),
        crate::registration::RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let catalog = |manifest| {
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![manifest])
            .expect("source-backed catalog")
    };
    let documents = vec![
        Arc::clone(&root),
        Arc::clone(&first_document),
        Arc::clone(&second_document),
    ];
    let forward = ProjectRegistrationFacts::try_new(
        world.clone(),
        documents.clone(),
        vec![fact.clone()],
        vec![catalog(first.clone()), catalog(second.clone())],
    )
    .expect("forward facts");
    let reverse = ProjectRegistrationFacts::try_new(
        world,
        documents,
        vec![fact],
        vec![catalog(second), catalog(first)],
    )
    .expect("reverse facts");
    let registered = register(&project, &forward, TypeCheckEnv::standard(), None)
        .expect("co-definitions register");

    let forward_index = CharacterDefinitionIndex::try_build_with_limits(
        &forward,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    )
    .expect("forward index");
    let reverse_index = CharacterDefinitionIndex::try_build_with_limits(
        &reverse,
        registered.symbols(),
        registered.environment(),
        CharacterDefinitionLimits::PRODUCTION,
    )
    .expect("reverse index");
    let descriptor = CharacterSymbolDescriptor::Owner { character: owner };
    let forward_sources = forward_index
        .declaration(&descriptor)
        .expect("forward owner")
        .sources()
        .cloned()
        .collect::<Vec<_>>();
    let reverse_sources = reverse_index
        .declaration(&descriptor)
        .expect("reverse owner")
        .sources()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(forward_sources, reverse_sources);
    assert_eq!(
        forward_index.source_revision(),
        reverse_index.source_revision()
    );
}
