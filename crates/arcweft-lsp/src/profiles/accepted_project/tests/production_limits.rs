use super::*;
use arcweft_lang_sema::registration::CharacterRegistrationLimits;

fn unavailable_seed(document: Arc<SourceDocument>) -> AcceptedSourceDocumentSeed {
    AcceptedSourceDocumentSeed::new(
        document,
        AcceptedSourceLocator::Unavailable,
        AcceptedSourceOwnership::Generated,
        AcceptedSourceAccess::Unknown,
    )
}

#[test]
fn production_document_limit_accepts_exact_and_rejects_one_over() {
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.documents())
        .expect("production document limit fits usize");
    let root = document(
        "arcweft-project://accepted/production-document-root.arcw",
        "fn main() -> Unit { () }\n",
    );
    let (hir, world) = project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let mut seeds = Vec::with_capacity(maximum + 1);
    seeds.push(seed(
        Arc::clone(&root),
        "file:///accepted/production-document-root.arcw",
    ));
    seeds.extend((1..maximum).map(|index| {
        unavailable_seed(document(
            &format!("arcweft-generated://accepted/limit-{index:04}.arcw"),
            "",
        ))
    }));

    let exact = AcceptedProjectSnapshot::try_new(Arc::clone(&hir), world.as_ref(), seeds.clone())
        .expect("4,096 accepted documents are inclusive");
    assert_eq!(
        exact.footprint().documents(),
        CharacterRegistrationLimits::PRODUCTION.documents()
    );

    seeds.push(unavailable_seed(document(
        "arcweft-generated://accepted/limit-one-over.arcw",
        "this source is rejected before any HIR parse or lower",
    )));
    assert!(matches!(
        AcceptedProjectSnapshot::try_new(hir, world.as_ref(), seeds),
        Err(AcceptedProjectSnapshotError::Limit {
            kind: AcceptedProjectLimitKind::Documents,
            observed,
            maximum: actual_maximum,
        }) if observed == CharacterRegistrationLimits::PRODUCTION.documents() + 1
            && actual_maximum == CharacterRegistrationLimits::PRODUCTION.documents()
    ));
}

#[test]
fn production_source_byte_limit_accepts_exact_and_rejects_one_over() {
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.source_bytes())
        .expect("production source-byte limit fits usize");
    let root = document(
        "arcweft-project://accepted/production-byte-root.arcw",
        "fn main() -> Unit { () }\n",
    );
    let (hir, world) = project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let generated_len = maximum - root.text().len();
    let exact_generated = document(
        "arcweft-generated://accepted/production-byte-padding.arcw",
        &"x".repeat(generated_len),
    );
    let exact = AcceptedProjectSnapshot::try_new(
        Arc::clone(&hir),
        world.as_ref(),
        vec![
            seed(
                Arc::clone(&root),
                "file:///accepted/production-byte-root.arcw",
            ),
            unavailable_seed(exact_generated),
        ],
    )
    .expect("8 MiB accepted source aggregate is inclusive");
    assert_eq!(
        exact.footprint().source_bytes(),
        CharacterRegistrationLimits::PRODUCTION.source_bytes()
    );

    let one_over_generated = document(
        "arcweft-generated://accepted/production-byte-one-over.arcw",
        &"x".repeat(generated_len + 1),
    );
    assert!(matches!(
        AcceptedProjectSnapshot::try_new(
            hir,
            world.as_ref(),
            vec![
                seed(root, "file:///accepted/production-byte-root.arcw"),
                unavailable_seed(one_over_generated),
            ],
        ),
        Err(AcceptedProjectSnapshotError::Limit {
            kind: AcceptedProjectLimitKind::SourceBytes,
            observed,
            maximum: actual_maximum,
        }) if observed == CharacterRegistrationLimits::PRODUCTION.source_bytes() + 1
            && actual_maximum == CharacterRegistrationLimits::PRODUCTION.source_bytes()
    ));
}

#[test]
fn source_byte_counter_overflow_rejects_before_registry_mutation() {
    let root = document(
        "arcweft-project://accepted/production-byte-overflow.arcw",
        "fn main() -> Unit { () }\n",
    );
    let mut builder = AcceptedSourceRegistryBuilder {
        source_bytes: u64::MAX,
        ..AcceptedSourceRegistryBuilder::default()
    };

    assert!(matches!(
        builder.insert(seed(root, "file:///accepted/production-byte-overflow.arcw",)),
        Err(AcceptedProjectSnapshotError::ArithmeticOverflow {
            counter: AcceptedProjectLimitKind::SourceBytes,
        })
    ));
    assert!(builder.identities_by_id.is_empty());
    assert!(builder.by_identity.is_empty());
    assert!(builder.by_uri.is_empty());
    assert_eq!(builder.source_bytes, u64::MAX);
}
