use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::identity::{
    HirDatabaseId, HirIdKind, HirModuleId, HirRevision, HirSnapshotId, HirTypedId, ItemId,
    RawHirId, TypeId,
};
use crate::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};

use super::{
    HirProofReturnAuthorityError, HirProofReturnHeader, HirProofReturnModuleLease,
    HirProofReturnProjectGeneration, HirProofReturnSemanticClass, HirProofReturnSemanticFact,
    HirProofReturnSemanticFactSet,
};

struct Fixture {
    database: HirDatabaseId,
    package: CallablePackageId,
    path: CanonicalModulePath,
    snapshot: HirSnapshotId,
    syntax_snapshot: arcweft_lang_syntax::attachment::SyntaxSnapshotId,
    document: Arc<SourceDocument>,
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
}

impl Fixture {
    fn new(database_raw: u64, source_id: &str) -> Self {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(source_id).unwrap(),
                SourceName::path("proof/return-authority.arcw"),
                "proof checked() -> Unit {}",
            )
            .unwrap(),
        );
        let mut syntax = SyntaxDatabase::try_new().unwrap();
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                Arc::clone(&document),
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .unwrap();
        let database = HirDatabaseId::from_raw_for_test(NonZeroU64::new(database_raw).unwrap());
        let module = HirModuleId::new(database, NonZeroU32::MIN);
        let snapshot = HirSnapshotId::new(module, HirRevision::INITIAL);
        let package = CallablePackageId::try_new("proof-return-tests").unwrap();
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            document.identity().id().clone(),
            "test",
        )
        .unwrap();
        let revision = ProjectSymbolRevision::try_for_documents([document.identity()]).unwrap();
        Self {
            database,
            package,
            path: CanonicalModulePath::crate_root(),
            snapshot,
            syntax_snapshot: parsed.snapshot_id().clone(),
            document,
            world,
            revision,
        }
    }

    fn generation(&self) -> Arc<HirProofReturnProjectGeneration> {
        HirProofReturnProjectGeneration::try_new(
            self.database,
            self.world.clone(),
            self.revision,
            [self.document.identity()],
            [HirProofReturnModuleLease::new(
                self.package.clone(),
                self.path.clone(),
                self.snapshot,
                self.syntax_snapshot.clone(),
                self.document.identity().clone(),
            )],
        )
        .unwrap()
    }

    fn item(&self, slot: u32) -> ItemId {
        typed_id(self.snapshot.module(), slot, HirIdKind::Item)
    }

    fn ty(&self, slot: u32) -> TypeId {
        typed_id(self.snapshot.module(), slot, HirIdKind::Type)
    }

    fn header(
        &self,
        generation: Arc<HirProofReturnProjectGeneration>,
        item: ItemId,
        ty: TypeId,
    ) -> HirProofReturnHeader {
        HirProofReturnHeader::try_new(
            generation,
            self.path.clone(),
            item,
            ty,
            self.document.start_span(),
        )
        .unwrap()
    }
}

fn typed_id<I: HirTypedId>(module: HirModuleId, slot: u32, kind: HirIdKind) -> I {
    I::from_raw(RawHirId::new(module, NonZeroU32::new(slot).unwrap(), kind))
}

#[test]
fn complete_fact_set_retains_exact_unit_classification() {
    let fixture = Fixture::new(1, "arcweft-test://proof/return-authority");
    let generation = fixture.generation();
    let header = fixture.header(generation.clone(), fixture.item(1), fixture.ty(2));
    let fact =
        HirProofReturnSemanticFact::new(header.clone(), HirProofReturnSemanticClass::Unit, 7);
    let facts =
        HirProofReturnSemanticFactSet::try_new(generation, [header.clone()], [fact]).unwrap();

    assert_eq!(facts.generation().validation_work(), 2);
    assert_eq!(facts.validation_work(), 3);
    assert_eq!(facts.semantic_work(), 7);
    assert_eq!(
        facts.class_for(&header),
        Ok(HirProofReturnSemanticClass::Unit)
    );
    assert!(HirProofReturnSemanticClass::Unit.admits_implicit_unit_tail());
    assert!(!HirProofReturnSemanticClass::NonUnit.admits_implicit_unit_tail());
    assert!(HirProofReturnSemanticClass::Poisoned.is_poisoned());
}

#[test]
fn fact_set_rejects_missing_duplicate_and_wrong_type_evidence() {
    let fixture = Fixture::new(2, "arcweft-test://proof/return-completeness");
    let generation = fixture.generation();
    let header = fixture.header(generation.clone(), fixture.item(1), fixture.ty(2));

    assert!(matches!(
        HirProofReturnSemanticFactSet::try_new(generation.clone(), [header.clone()], []),
        Err(HirProofReturnAuthorityError::MissingFact { item }) if item == header.item()
    ));
    assert!(matches!(
        HirProofReturnSemanticFactSet::try_new(
            generation.clone(),
            [header.clone(), header.clone()],
            [],
        ),
        Err(HirProofReturnAuthorityError::DuplicateHeader { item }) if item == header.item()
    ));

    let wrong_type = fixture.header(generation.clone(), header.item(), fixture.ty(3));
    let wrong_fact =
        HirProofReturnSemanticFact::new(wrong_type, HirProofReturnSemanticClass::NonUnit, 1);
    assert!(matches!(
        HirProofReturnSemanticFactSet::try_new(generation, [header], [wrong_fact]),
        Err(HirProofReturnAuthorityError::FactBindingMismatch { .. })
    ));
}

#[test]
fn fact_set_rejects_semantic_work_overflow() {
    let fixture = Fixture::new(8, "arcweft-test://proof/return-work");
    let generation = fixture.generation();
    let first = fixture.header(generation.clone(), fixture.item(1), fixture.ty(2));
    let second = fixture.header(generation.clone(), fixture.item(3), fixture.ty(4));
    let first_fact =
        HirProofReturnSemanticFact::new(first.clone(), HirProofReturnSemanticClass::Unit, u64::MAX);
    let second_fact =
        HirProofReturnSemanticFact::new(second.clone(), HirProofReturnSemanticClass::Unit, 1);

    assert_eq!(
        HirProofReturnSemanticFactSet::try_new(
            generation,
            [first, second],
            [first_fact, second_fact],
        )
        .unwrap_err(),
        HirProofReturnAuthorityError::WorkOverflow
    );
}

#[test]
fn fact_set_rejects_foreign_generation_item_and_source_bindings() {
    let fixture = Fixture::new(3, "arcweft-test://proof/return-binding");
    let expected_generation = fixture.generation();
    let equal_but_foreign_generation = fixture.generation();
    let expected = fixture.header(expected_generation.clone(), fixture.item(1), fixture.ty(2));
    let foreign = fixture.header(equal_but_foreign_generation, fixture.item(1), fixture.ty(2));
    let foreign_fact =
        HirProofReturnSemanticFact::new(foreign, HirProofReturnSemanticClass::Unit, 1);
    assert_eq!(
        HirProofReturnSemanticFactSet::try_new(
            expected_generation.clone(),
            [expected.clone()],
            [foreign_fact],
        )
        .unwrap_err(),
        HirProofReturnAuthorityError::ForeignGeneration
    );

    let unexpected = fixture.header(expected_generation.clone(), fixture.item(4), fixture.ty(5));
    let unexpected_fact =
        HirProofReturnSemanticFact::new(unexpected, HirProofReturnSemanticClass::Unit, 1);
    assert!(matches!(
        HirProofReturnSemanticFactSet::try_new(expected_generation, [expected], [unexpected_fact],),
        Err(HirProofReturnAuthorityError::UnexpectedFact { .. })
    ));

    let foreign_document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://proof/foreign-source").unwrap(),
        SourceName::path("proof/foreign.arcw"),
        "",
    )
    .unwrap();
    assert!(matches!(
        HirProofReturnHeader::try_new(
            fixture.generation(),
            fixture.path.clone(),
            fixture.item(1),
            fixture.ty(2),
            foreign_document.start_span(),
        ),
        Err(HirProofReturnAuthorityError::WrongSource { .. })
    ));
}

#[test]
fn generation_rejects_foreign_database_and_stale_symbol_revision() {
    let fixture = Fixture::new(4, "arcweft-test://proof/return-generation");
    let foreign_database = HirDatabaseId::from_raw_for_test(NonZeroU64::new(5).unwrap());
    let lease = HirProofReturnModuleLease::new(
        fixture.package.clone(),
        fixture.path.clone(),
        fixture.snapshot,
        fixture.syntax_snapshot.clone(),
        fixture.document.identity().clone(),
    );
    assert!(matches!(
        HirProofReturnProjectGeneration::try_new(
            foreign_database,
            fixture.world.clone(),
            fixture.revision,
            [fixture.document.identity()],
            [lease.clone()],
        ),
        Err(super::HirProofReturnGenerationError::WrongDatabase { .. })
    ));

    let other = SourceDocument::try_new(
        fixture.document.identity().id().clone(),
        fixture.document.display_name().clone(),
        "changed",
    )
    .unwrap();
    let stale = ProjectSymbolRevision::try_for_documents([other.identity()]).unwrap();
    assert!(matches!(
        HirProofReturnProjectGeneration::try_new(
            fixture.database,
            fixture.world,
            stale,
            [fixture.document.identity()],
            [lease],
        ),
        Err(super::HirProofReturnGenerationError::WrongRevision { .. })
    ));
}

#[test]
fn generation_uses_full_symbol_revision_and_requires_exact_module_membership() {
    let fixture = Fixture::new(9, "arcweft-test://proof/return-symbol-inventory");
    let environment = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://proof/return-environment").unwrap(),
        SourceName::path("proof/return-environment.json"),
        "environment",
    )
    .unwrap();
    let revision = ProjectSymbolRevision::try_for_documents([
        fixture.document.identity(),
        environment.identity(),
    ])
    .unwrap();
    let lease = HirProofReturnModuleLease::new(
        fixture.package.clone(),
        fixture.path.clone(),
        fixture.snapshot,
        fixture.syntax_snapshot.clone(),
        fixture.document.identity().clone(),
    );
    let generation = HirProofReturnProjectGeneration::try_new(
        fixture.database,
        fixture.world.clone(),
        revision,
        [fixture.document.identity(), environment.identity()],
        [lease.clone()],
    )
    .expect("non-module registration sources belong to the symbol generation");
    assert_eq!(generation.revision(), revision);
    assert_eq!(generation.validation_work(), 3);

    let environment_only =
        ProjectSymbolRevision::try_for_documents([environment.identity()]).unwrap();
    assert!(matches!(
        HirProofReturnProjectGeneration::try_new(
            fixture.database,
            fixture.world,
            environment_only,
            [environment.identity()],
            [lease],
        ),
        Err(super::HirProofReturnGenerationError::MissingModuleSource { .. })
    ));
}

#[test]
fn module_transaction_rejects_stale_hir_syntax_and_source_leases() {
    let fixture = Fixture::new(6, "arcweft-test://proof/return-transaction");
    let other = Fixture::new(7, "arcweft-test://proof/return-transaction-other");
    let generation = fixture.generation();

    assert!(matches!(
        generation.validate_module_transaction(
            &fixture.package,
            &fixture.path,
            other.snapshot,
            &fixture.syntax_snapshot,
            fixture.document.identity(),
        ),
        Err(HirProofReturnAuthorityError::WrongHirSnapshot { .. })
    ));
    assert!(matches!(
        generation.validate_module_transaction(
            &fixture.package,
            &fixture.path,
            fixture.snapshot,
            &other.syntax_snapshot,
            fixture.document.identity(),
        ),
        Err(HirProofReturnAuthorityError::WrongSyntaxSnapshot { .. })
    ));
    assert!(matches!(
        generation.validate_module_transaction(
            &fixture.package,
            &fixture.path,
            fixture.snapshot,
            &fixture.syntax_snapshot,
            other.document.identity(),
        ),
        Err(HirProofReturnAuthorityError::WrongSource { .. })
    ));
}
