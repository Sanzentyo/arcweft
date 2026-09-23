use super::*;
use std::sync::Arc;

use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::ModulePathRoot;
use arcweft_lang_syntax::ast::symbol_path::{ProjectSymbolPath, ProjectSymbolSegment};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::callable::{AdapterPackageId, RustItemPath, RustPackageProvenance};
use crate::env::rust_metadata::{
    RustStructMetadataInput, RustTypeMetadataPublicationIdentity, RustTypeMetadataPublicationKind,
    RustTypeParameterPublicationInput,
};
use crate::env::{
    TypeCheckEnv,
    nominal::{
        AcceptedNominalOrigin, AcceptedNominalOwnerId, AcceptedNominalRecord, RustPackageId,
    },
};
use crate::nominal::{NominalAggregationLimits, NominalResolutionLimits};
use crate::registration::{AcceptedNominalSource, AcceptedNominalVisibilityIndex};

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://rust-metadata-join").unwrap(),
        SourceName::Generated,
        text,
    )
    .unwrap()
}

fn input() -> RustTypeMetadataPublicationInput {
    let document = document("Rank");
    let source = document.span(SourceRange::new(0, 4)).unwrap();
    let package = RustPackageId::try_new("fixture").unwrap();
    let path = arcweft_lang_syntax::types::TypePath::from(
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new("Rank").unwrap()],
        )
        .unwrap(),
    );
    let id = AcceptedNominalId::new(
        AcceptedNominalOwnerId::RustPackage(package.clone()),
        path.clone(),
    );
    let rust_item = RustItemPath::try_new("fixture::Rank").unwrap();
    RustTypeMetadataPublicationInput::new(
        RustTypeMetadataPublicationIdentity::new(
            EnvironmentPublicationItemId::RustType {
                adapter: AdapterPackageId::try_new("fixture-adapter").unwrap(),
                package: package.clone(),
                rust_item: rust_item.clone(),
                accepted_path: path,
            },
            id,
            package,
            RustPackageProvenance::try_new("fixture", "1.0.0", None).unwrap(),
            rust_item,
        ),
        Box::<[RustTypeParameterPublicationInput]>::default(),
        RustTypeMetadataPublicationKind::Struct {
            shape: RustStructMetadataInput::Unit,
        },
        source,
    )
}

fn world(
    input: &RustTypeMetadataPublicationInput,
    record: AcceptedNominalRecord,
    visibility: AcceptedNominalVisibilityIndex,
) -> AcceptedNominalWorld {
    let environment = TypeCheckEnv::new().try_with_nominal_record(record).unwrap();
    AcceptedNominalWorld::new(
        Arc::new(environment),
        ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("fixture-world").unwrap(),
            input.source().source().id().clone(),
            "test",
        )
        .unwrap(),
        ProjectSymbolRevision::try_for_documents([input.source().source()]).unwrap(),
        BTreeMap::new(),
        visibility,
    )
}

fn record(input: &RustTypeMetadataPublicationInput) -> AcceptedNominalRecord {
    AcceptedNominalRecord::try_new_rust_adt(input.id().clone(), 0, input.source().clone()).unwrap()
}

fn publication(
    input: &RustTypeMetadataPublicationInput,
) -> BTreeMap<AcceptedNominalId, AcceptedNominalSource> {
    BTreeMap::from([(
        input.id().clone(),
        AcceptedNominalSource::new(input.source().clone(), input.item().clone()),
    )])
}

fn visible_world(input: &RustTypeMetadataPublicationInput) -> AcceptedNominalWorld {
    world(
        input,
        record(input),
        AcceptedNominalVisibilityIndex::from_parts(publication(input), BTreeMap::new()),
    )
}

#[test]
fn complete_join_projects_the_same_publication_and_keeps_world_unchanged() {
    let input = input();
    for visibility in [
        AcceptedNominalVisibilityIndex::from_parts(publication(&input), BTreeMap::new()),
        AcceptedNominalVisibilityIndex::from_parts(BTreeMap::new(), publication(&input)),
    ] {
        let world = world(&input, record(&input), visibility);
        let stamp = world.stamp();
        let catalog = world
            .join_rust_metadata(
                std::slice::from_ref(&input),
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION,
            )
            .unwrap()
            .project()
            .unwrap();
        let accepted = catalog.get(input.id()).unwrap();
        assert_eq!(accepted.item(), input.item());
        assert_eq!(accepted.source(), input.source());
        assert_eq!(world.stamp(), stamp);
        let instantiated = catalog
            .instantiate(&crate::types::AcceptedNominalType::new(
                input.id().clone(),
                Box::<[crate::types::TypeKind]>::default(),
            ))
            .unwrap();
        assert_eq!(instantiated.item(), input.item());
    }
}

#[test]
fn work_limits_precede_join_errors_and_are_inclusive() {
    let input = input();
    let world = visible_world(&input);
    // The row header and the four-byte codec-policy name cost 1 + (1 + 4).
    let limits = NominalAggregationLimits::try_new(1, 1, 6).unwrap();
    world
        .join_rust_metadata(
            std::slice::from_ref(&input),
            NominalResolutionLimits::PRODUCTION,
            limits,
        )
        .unwrap()
        .project()
        .unwrap();
    let insufficient = NominalAggregationLimits::try_new(1, 1, 5).unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(
                std::slice::from_ref(&input),
                NominalResolutionLimits::PRODUCTION,
                insufficient,
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::AggregateLimit {
            kind: NominalAggregationLimitKind::WorkPerProject,
            observed: 6,
            maximum: 5
        }
    ));
    let duplicate = [input.clone(), input.clone()];
    assert!(matches!(
        world
            .join_rust_metadata(&duplicate, NominalResolutionLimits::PRODUCTION, limits)
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::AggregateLimit {
            kind: NominalAggregationLimitKind::WorkPerProject,
            observed: 7,
            maximum: 6
        }
    ));
    let sufficient = NominalAggregationLimits::try_new(1, 1, 12).unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(&duplicate, NominalResolutionLimits::PRODUCTION, sufficient)
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::DuplicateMetadata { .. }
    ));
    let mut deep = input.clone();
    let node = crate::registration::EnvironmentTypeProjectionNode::new(
        input.source().clone(),
        crate::registration::EnvironmentTypeProjectionKind::Bool,
    );
    deep.kind = RustTypeMetadataPublicationKind::Newtype {
        inner: crate::registration::EnvironmentTypeProjectionNode::new(
            input.source().clone(),
            crate::registration::EnvironmentTypeProjectionKind::Option(Box::new(node)),
        ),
    };
    let shallow = NominalResolutionLimits::try_new(1, 1, 1, 1, 1, 1, 1, 65_536).unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(&[deep], shallow, NominalAggregationLimits::PRODUCTION)
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::ReferenceLimit {
            kind: NominalResolutionLimitKind::TypeNodesPerReference,
            observed: 2,
            maximum: 1
        }
    ));
}

#[test]
fn incomplete_and_foreign_metadata_cannot_produce_a_join_proof() {
    let original = input();
    let world = visible_world(&original);
    let stamp = world.stamp();
    assert!(matches!(
        world
            .join_rust_metadata(
                &[],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::MissingMetadata
    ));
    assert!(matches!(
        world
            .join_rust_metadata(
                &[original.clone(), original.clone()],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::DuplicateMetadata { .. }
    ));

    let mut different_item = original.clone();
    let EnvironmentPublicationItemId::RustType { adapter, .. } = &mut different_item.item else {
        unreachable!()
    };
    *adapter = AdapterPackageId::try_new("foreign-adapter").unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(
                &[different_item],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::PublicationMismatch { .. }
    ));

    let mut different_source = original.clone();
    different_source.source = document("Rank revised")
        .span(SourceRange::new(0, 4))
        .unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(
                &[different_source],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::SourceMismatch { .. }
    ));

    let mut different_package = original.clone();
    different_package.package = RustPackageId::try_new("foreign-package").unwrap();
    assert!(matches!(
        world
            .join_rust_metadata(
                &[different_package],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::OwnerMismatch
    ));

    let mut different_arity = original.clone();
    different_arity.parameters = Box::new([RustTypeParameterPublicationInput::new(
        arcweft_rust_abi::ArcweftRustTypeParameterIndex::try_from_usize(0).unwrap(),
        "T".to_owned(),
        original.source().clone(),
    )]);
    assert!(matches!(
        world
            .join_rust_metadata(
                &[different_arity],
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::ArityMismatch {
            expected: 0,
            actual: 1
        }
    ));
    assert_eq!(world.stamp(), stamp);
}

#[test]
fn opaque_or_unpublished_declarations_cannot_claim_rust_metadata() {
    let input = input();
    let opaque = AcceptedNominalRecord::try_new_opaque(
        input.id().clone(),
        0,
        arcweft_core::pattern::RuntimeOpaqueTypeProducerId::try_new("fixture.opaque").unwrap(),
        arcweft_core::value::RuntimeOpaqueValueClass::Plain,
        arcweft_core::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
        AcceptedNominalOrigin::Adapter,
        Some(input.source().clone()),
    )
    .unwrap();
    let opaque_world = world(
        &input,
        opaque,
        AcceptedNominalVisibilityIndex::from_parts(publication(&input), BTreeMap::new()),
    );
    assert!(matches!(
        opaque_world
            .join_rust_metadata(
                std::slice::from_ref(&input),
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::NonStructuralDeclaration
    ));
    let unpublished = world(
        &input,
        record(&input),
        AcceptedNominalVisibilityIndex::default(),
    );
    assert!(matches!(
        unpublished
            .join_rust_metadata(
                std::slice::from_ref(&input),
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::MissingPublication
    ));
    let ambiguous = world(
        &input,
        record(&input),
        AcceptedNominalVisibilityIndex::from_parts(publication(&input), publication(&input)),
    );
    assert!(matches!(
        ambiguous
            .join_rust_metadata(
                std::slice::from_ref(&input),
                NominalResolutionLimits::PRODUCTION,
                NominalAggregationLimits::PRODUCTION
            )
            .unwrap_err()
            .kind(),
        RustMetadataJoinErrorKind::AmbiguousPublication
    ));
}
