use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::item::HirItemKind;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{HirProject, HirProjectModule};
use arcweft_lang_hir::proof_return::{
    HirProofReturnHeader, HirProofReturnModuleLease, HirProofReturnProjectGeneration,
    HirProofReturnSemanticClass, HirProofReturnSemanticFactSet,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::env::TypeCheckEnv;
use crate::nominal::{
    GenericTypeScope, NominalResolutionLimits, ResolvedTypeRefOutcome, SelfTypeScope,
    TypeResolutionInput, resolve_type_ref,
};
use crate::registration::{
    CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
    ProofReturnRegistrationRequest, RegisteredSemanticWorld,
};

use super::{classify_proof_return, classify_proof_return_project};

struct Fixture {
    database_id: arcweft_lang_hir::identity::HirDatabaseId,
    package: CallablePackageId,
    path: CanonicalModulePath,
    document: Arc<SourceDocument>,
    module: Arc<arcweft_lang_hir::module::HirModule>,
    project: HirProject,
    registered: RegisteredSemanticWorld,
}

struct MultiFixture {
    database_id: arcweft_lang_hir::identity::HirDatabaseId,
    package: CallablePackageId,
    project: HirProject,
    registered: RegisteredSemanticWorld,
    modules: BTreeMap<
        CanonicalModulePath,
        (
            Arc<SourceDocument>,
            Arc<arcweft_lang_hir::module::HirModule>,
        ),
    >,
}

fn publish_headerless_project_modules(
    database: &mut HirDatabase,
    package: &CallablePackageId,
    modules: &[(CanonicalModulePath, Arc<SourceDocument>, ParsedSource)],
) -> BTreeMap<CanonicalModulePath, Arc<arcweft_lang_hir::module::HirModule>> {
    let root_document = modules
        .iter()
        .find(|(path, _, _)| path == &CanonicalModulePath::crate_root())
        .map(|(_, document, _)| document)
        .expect("fixture root module");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        root_document.identity().id().clone(),
        "proof-return-fixture-lowering",
    )
    .unwrap();
    let revision = ProjectSymbolRevision::try_for_documents(
        modules.iter().map(|(_, document, _)| document.identity()),
    )
    .unwrap();
    let transaction = database
        .stage_proof_return_project(
            modules.iter().map(|(path, document, parsed)| {
                LoweringRequest::try_new(
                    HirModuleKey::new(package.clone(), path.clone(), document.identity().clone()),
                    parsed,
                )
                .unwrap()
            }),
            world,
            revision,
            modules.iter().map(|(_, document, _)| document.identity()),
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .unwrap();
    let facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("nominal fixtures have no authored Proof returns");
    transaction
        .publish_with_semantic_facts(database, facts)
        .unwrap()
        .into_iter()
        .map(|output| {
            let path = output.module().key().path().clone();
            (path, output.into_module())
        })
        .collect()
}

impl Fixture {
    fn new(source: &str) -> Self {
        let package = CallablePackageId::try_new("proof-return-sema-tests").unwrap();
        let path = CanonicalModulePath::crate_root();
        let name = SourceName::path("proof/return-sema.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://proof/return-sema").unwrap(),
                name.clone(),
                source,
            )
            .unwrap(),
        );
        let mut syntax = SyntaxDatabase::try_new().unwrap();
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(name),
                Arc::clone(&document),
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let mut database = HirDatabase::try_new().unwrap();
        let staged = vec![(path.clone(), Arc::clone(&document), parsed)];
        let module = publish_headerless_project_modules(&mut database, &package, &staged)
            .remove(&path)
            .unwrap();
        let database_id = database.database_id();
        let project_module = HirProjectModule::try_new(
            &database,
            &package,
            &path,
            document.identity(),
            Arc::clone(&module),
        )
        .unwrap();
        let project = HirProject::try_new(&database, package.clone(), [project_module]).unwrap();
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            document.identity().id().clone(),
            "test",
        )
        .unwrap();
        let registration = ProjectRegistrationFacts::try_new(
            world,
            vec![Arc::clone(&document)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            project.view(),
            &registration,
            None,
        ))
        .unwrap();
        Self {
            database_id,
            package,
            path,
            document,
            module,
            project,
            registered,
        }
    }

    fn alias(
        &self,
        name: &str,
    ) -> (
        arcweft_lang_hir::identity::ItemId,
        arcweft_lang_hir::identity::TypeId,
    ) {
        self.project
            .view()
            .items()
            .find_map(|item| {
                let HirItemKind::TypeAlias(alias) = item.item().kind() else {
                    return None;
                };
                (alias.name().resolved()?.as_str() == name).then_some((item.id(), alias.target()))
            })
            .unwrap_or_else(|| panic!("missing type alias `{name}`"))
    }

    fn generation(&self) -> Arc<HirProofReturnProjectGeneration> {
        HirProofReturnProjectGeneration::try_new(
            self.database_id,
            self.registered.symbols().world().clone(),
            *self.registered.symbols().revision(),
            [self.document.identity()],
            [HirProofReturnModuleLease::new(
                self.package.clone(),
                self.path.clone(),
                self.module.snapshot_id(),
                self.module.provenance().syntax_snapshot().clone(),
                self.document.identity().clone(),
            )],
        )
        .unwrap()
    }

    fn report(
        &self,
        root: arcweft_lang_hir::identity::TypeId,
    ) -> crate::nominal::TypeResolutionReport {
        let generics = GenericTypeScope::empty();
        let input = TypeResolutionInput::accepted(
            root,
            &self.module,
            self.project.view(),
            self.registered.symbols(),
            self.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap();
        resolve_type_ref(&input).unwrap()
    }

    fn detached_report(
        &self,
        root: arcweft_lang_hir::identity::TypeId,
    ) -> crate::nominal::TypeResolutionReport {
        let generics = GenericTypeScope::empty();
        let environment = TypeCheckEnv::standard();
        let input = TypeResolutionInput::detached(
            root,
            &self.module,
            &environment,
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap();
        resolve_type_ref(&input).unwrap()
    }

    fn classify(&self, alias: &str) -> HirProofReturnSemanticClass {
        let (item, ty) = self.alias(alias);
        let header = HirProofReturnHeader::try_new(
            self.generation(),
            self.path.clone(),
            item,
            ty,
            self.document.start_span(),
        )
        .unwrap();
        let generics = GenericTypeScope::empty();
        let input = TypeResolutionInput::accepted(
            ty,
            &self.module,
            self.project.view(),
            self.registered.symbols(),
            self.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap();
        classify_proof_return(header, &input)
            .unwrap()
            .fact()
            .class()
    }
}

impl MultiFixture {
    fn new(sources: &[(&str, &str)]) -> Self {
        let package = CallablePackageId::try_new("proof-return-multi-tests").unwrap();
        let mut syntax = SyntaxDatabase::try_new().unwrap();
        let mut database = HirDatabase::try_new().unwrap();
        let mut staged = Vec::new();
        for (path, source) in sources {
            let module_path = module_path(path);
            let file = if path.is_empty() {
                "root".to_owned()
            } else {
                path.replace('.', "/")
            };
            let name = SourceName::path(format!("proof/{file}.arcw"));
            let document = Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new(format!("arcweft-test://proof/return-multi/{file}"))
                        .unwrap(),
                    name.clone(),
                    *source,
                )
                .unwrap(),
            );
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(name),
                    Arc::clone(&document),
                    arcweft_lang_syntax::parser::ParseOptions::default(),
                )
                .unwrap();
            assert!(
                parsed.diagnostics().is_empty(),
                "{:?}",
                parsed.diagnostics()
            );
            staged.push((module_path, document, parsed));
        }
        let published = publish_headerless_project_modules(&mut database, &package, &staged);
        let modules = staged
            .into_iter()
            .map(|(path, document, _)| {
                let module = Arc::clone(&published[&path]);
                (path, (document, module))
            })
            .collect::<BTreeMap<_, _>>();
        let database_id = database.database_id();
        let project_modules = modules
            .iter()
            .map(|(path, (document, module))| {
                HirProjectModule::try_new(
                    &database,
                    &package,
                    path,
                    document.identity(),
                    Arc::clone(module),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let project = HirProject::try_new(&database, package.clone(), project_modules).unwrap();
        let root = modules.get(&CanonicalModulePath::crate_root()).unwrap();
        let world =
            ProjectSymbolWorldId::try_new(package.clone(), root.0.identity().id().clone(), "test")
                .unwrap();
        let registration = ProjectRegistrationFacts::try_new(
            world,
            modules
                .values()
                .map(|(document, _)| Arc::clone(document))
                .collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            project.view(),
            &registration,
            None,
        ))
        .unwrap();
        Self {
            database_id,
            package,
            project,
            registered,
            modules,
        }
    }

    fn alias(
        &self,
        module: &CanonicalModulePath,
        name: &str,
    ) -> (
        arcweft_lang_hir::identity::ItemId,
        arcweft_lang_hir::identity::TypeId,
    ) {
        self.project
            .view()
            .items()
            .filter(|item| item.module_path() == module)
            .find_map(|item| {
                let HirItemKind::TypeAlias(alias) = item.item().kind() else {
                    return None;
                };
                (alias.name().resolved()?.as_str() == name).then_some((item.id(), alias.target()))
            })
            .unwrap_or_else(|| panic!("missing type alias `{name}` in `{module}`"))
    }

    fn generation(&self) -> Arc<HirProofReturnProjectGeneration> {
        HirProofReturnProjectGeneration::try_new(
            self.database_id,
            self.registered.symbols().world().clone(),
            *self.registered.symbols().revision(),
            self.modules
                .values()
                .map(|(document, _)| document.identity()),
            self.modules.iter().map(|(path, (document, module))| {
                HirProofReturnModuleLease::new(
                    self.package.clone(),
                    path.clone(),
                    module.snapshot_id(),
                    module.provenance().syntax_snapshot().clone(),
                    document.identity().clone(),
                )
            }),
        )
        .unwrap()
    }

    fn classify(
        &self,
        module_path: &CanonicalModulePath,
        alias: &str,
    ) -> HirProofReturnSemanticClass {
        let (item, ty) = self.alias(module_path, alias);
        let (document, module) = self.modules.get(module_path).unwrap();
        let generics = GenericTypeScope::empty();
        let input = TypeResolutionInput::accepted(
            ty,
            module,
            self.project.view(),
            self.registered.symbols(),
            self.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap();
        let header = HirProofReturnHeader::try_new(
            self.generation(),
            module_path.clone(),
            item,
            ty,
            document.start_span(),
        )
        .unwrap();
        classify_proof_return(header, &input)
            .unwrap()
            .fact()
            .class()
    }
}

fn module_path(path: &str) -> CanonicalModulePath {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .fold(CanonicalModulePath::crate_root(), |module, segment| {
            module.join(ModuleSegment::new(segment).unwrap())
        })
}

#[test]
fn local_alias_chain_and_generic_alias_to_unit_are_unit() {
    let fixture = Fixture::new(
        "type Base = Unit\n\
         type Chain = Base\n\
         type Identity<T> = T\n\
         type GenericUnit = Identity<Unit>\n",
    );
    assert_eq!(fixture.classify("Base"), HirProofReturnSemanticClass::Unit);
    assert_eq!(fixture.classify("Chain"), HirProofReturnSemanticClass::Unit);
    assert_eq!(
        fixture.classify("GenericUnit"),
        HirProofReturnSemanticClass::Unit
    );
}

#[test]
fn authoritative_non_unit_stays_non_unit() {
    let fixture = Fixture::new("struct ResultRecord {}\ntype ResultType = ResultRecord\n");
    assert_eq!(
        fixture.classify("ResultType"),
        HirProofReturnSemanticClass::NonUnit
    );
}

#[test]
fn unknown_and_cyclic_alias_resolution_are_poisoned() {
    let unknown = Fixture::new("type ResultType = Missing\n");
    let (_, unknown_ty) = unknown.alias("ResultType");
    assert!(matches!(
        unknown.report(unknown_ty).outcome(),
        ResolvedTypeRefOutcome::Poisoned(_)
    ));
    assert_eq!(
        unknown.classify("ResultType"),
        HirProofReturnSemanticClass::Poisoned
    );

    let cyclic = Fixture::new("type First = Second\ntype Second = First\n");
    assert_eq!(
        cyclic.classify("First"),
        HirProofReturnSemanticClass::Poisoned
    );
}

#[test]
fn detached_project_nominal_resolution_cannot_publish_a_fact() {
    let fixture = Fixture::new(
        "struct ProjectType {}\n\
         type ResultType = ProjectType\n",
    );
    let (item, ty) = fixture.alias("ResultType");
    let report = fixture.detached_report(ty);
    assert!(matches!(
        report.outcome(),
        ResolvedTypeRefOutcome::Detached(_)
    ));
    let header = HirProofReturnHeader::try_new(
        fixture.generation(),
        fixture.path.clone(),
        item,
        ty,
        fixture.document.start_span(),
    )
    .unwrap();
    let generics = GenericTypeScope::empty();
    let environment = TypeCheckEnv::standard();
    let input = TypeResolutionInput::detached(
        ty,
        &fixture.module,
        &environment,
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(
        classify_proof_return(header, &input).unwrap_err(),
        super::ProofReturnClassificationError::DetachedWorld
    );
}

#[test]
fn classifier_rejects_an_input_for_another_return_type() {
    let fixture = Fixture::new("type UnitResult = Unit\ntype BoolResult = Bool\n");
    let (item, unit_ty) = fixture.alias("UnitResult");
    let (_, bool_ty) = fixture.alias("BoolResult");
    let header = HirProofReturnHeader::try_new(
        fixture.generation(),
        fixture.path.clone(),
        item,
        unit_ty,
        fixture.document.start_span(),
    )
    .unwrap();
    let generics = GenericTypeScope::empty();
    let input = TypeResolutionInput::accepted(
        bool_ty,
        &fixture.module,
        fixture.project.view(),
        fixture.registered.symbols(),
        fixture.registered.environment().nominal_world(),
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    )
    .unwrap();
    assert!(matches!(
        classify_proof_return(header, &input),
        Err(super::ProofReturnClassificationError::WrongReturnType { .. })
    ));
}

#[test]
fn classifier_rejects_an_equal_world_from_another_hir_database() {
    let source = "type ResultType = Unit\n";
    let fixture = Fixture::new(source);
    let foreign_database = Fixture::new(source);
    assert_eq!(
        fixture.registered.symbols().world(),
        foreign_database.registered.symbols().world()
    );
    assert_eq!(
        fixture.registered.symbols().revision(),
        foreign_database.registered.symbols().revision()
    );

    let (item, ty) = fixture.alias("ResultType");
    let (_, foreign_ty) = foreign_database.alias("ResultType");
    let header = HirProofReturnHeader::try_new(
        fixture.generation(),
        fixture.path.clone(),
        item,
        ty,
        fixture.document.start_span(),
    )
    .unwrap();
    let generics = GenericTypeScope::empty();
    let input = TypeResolutionInput::accepted(
        foreign_ty,
        &foreign_database.module,
        foreign_database.project.view(),
        foreign_database.registered.symbols(),
        foreign_database.registered.environment().nominal_world(),
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    )
    .unwrap();

    assert!(matches!(
        classify_proof_return(header, &input),
        Err(super::ProofReturnClassificationError::GenerationLease(
            arcweft_lang_hir::proof_return::HirProofReturnAuthorityError::WrongHirSnapshot { .. }
        ))
    ));
}

#[test]
fn classifier_rejects_foreign_symbol_world_and_stale_revision() {
    let fixture = Fixture::new("type ResultType = Unit\n");
    let (item, ty) = fixture.alias("ResultType");
    let foreign_world = MultiFixture::new(&[("", "type ResultType = Unit\n")]);
    let root = CanonicalModulePath::crate_root();
    let (_, foreign_ty) = foreign_world.alias(&root, "ResultType");
    let (_, foreign_module) = foreign_world.modules.get(&root).unwrap();
    let foreign_generics = GenericTypeScope::empty();
    let foreign_input = TypeResolutionInput::accepted(
        foreign_ty,
        foreign_module,
        foreign_world.project.view(),
        foreign_world.registered.symbols(),
        foreign_world.registered.environment().nominal_world(),
        &foreign_generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    )
    .unwrap();
    let header = HirProofReturnHeader::try_new(
        fixture.generation(),
        fixture.path.clone(),
        item,
        ty,
        fixture.document.start_span(),
    )
    .unwrap();
    assert!(matches!(
        classify_proof_return(header, &foreign_input),
        Err(super::ProofReturnClassificationError::WrongSymbolWorld { .. })
    ));

    let stale = Fixture::new("type ResultType = Bool\n");
    let (_, stale_ty) = stale.alias("ResultType");
    let stale_generics = GenericTypeScope::empty();
    let stale_input = TypeResolutionInput::accepted(
        stale_ty,
        &stale.module,
        stale.project.view(),
        stale.registered.symbols(),
        stale.registered.environment().nominal_world(),
        &stale_generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    )
    .unwrap();
    let header = HirProofReturnHeader::try_new(
        fixture.generation(),
        fixture.path.clone(),
        item,
        ty,
        fixture.document.start_span(),
    )
    .unwrap();
    assert!(matches!(
        classify_proof_return(header, &stale_input),
        Err(super::ProofReturnClassificationError::WrongSymbolRevision { .. })
    ));
}

#[test]
fn imported_and_root_qualified_aliases_to_unit_are_unit() {
    let fixture = MultiFixture::new(&[
        (
            "",
            "use crate.child.ImportedUnit\n\
             type ThroughImport = ImportedUnit\n\
             type ThroughQualified = crate.child.ImportedUnit\n",
        ),
        ("child", "pub type ImportedUnit = Unit\n"),
    ]);
    let root = CanonicalModulePath::crate_root();
    assert_eq!(
        fixture.classify(&root, "ThroughImport"),
        HirProofReturnSemanticClass::Unit
    );
    assert_eq!(
        fixture.classify(&root, "ThroughQualified"),
        HirProofReturnSemanticClass::Unit
    );
}

#[test]
fn inaccessible_and_ambiguous_alias_targets_are_poisoned() {
    let inaccessible = MultiFixture::new(&[
        ("", "type ResultType = crate.child.Hidden\n"),
        ("child", "type Hidden = Unit\n"),
    ]);
    let root = CanonicalModulePath::crate_root();
    assert_eq!(
        inaccessible.classify(&root, "ResultType"),
        HirProofReturnSemanticClass::Poisoned
    );

    let ambiguous = MultiFixture::new(&[
        (
            "",
            "use crate.left.*\nuse crate.right.*\ntype ResultType = CollisionUnit\n",
        ),
        ("left", "pub type CollisionUnit = Unit\n"),
        ("right", "pub type CollisionUnit = Unit\n"),
    ]);
    assert_eq!(
        ambiguous.classify(&root, "ResultType"),
        HirProofReturnSemanticClass::Poisoned
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the atomic multi-module Proof publication regression constructs and verifies the complete staged project in one test"
)]
fn staged_multi_module_project_classifies_every_proof_before_atomic_publication() {
    let package = CallablePackageId::try_new("proof-return-staged-project-tests").unwrap();
    let root = CanonicalModulePath::crate_root();
    let aliases = module_path("aliases");
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let sources = [
        (
            root.clone(),
            "root",
            "use crate.aliases.ProofUnit\n\
             type Identity<T> = T\n\
             proof root_checked<T>() -> Identity<ProofUnit> {}\n",
        ),
        (
            aliases.clone(),
            "aliases",
            "pub type ProofUnit = Unit\nproof alias_checked() -> Unit {}\n",
        ),
    ];
    let modules = sources
        .into_iter()
        .map(|(path, label, source)| {
            let name = SourceName::path(format!("proof/{label}.arcw"));
            let document = Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new(format!(
                        "arcweft-test://proof/return-staged/{label}"
                    ))
                    .unwrap(),
                    name.clone(),
                    source,
                )
                .unwrap(),
            );
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(name),
                    Arc::clone(&document),
                    arcweft_lang_syntax::parser::ParseOptions::default(),
                )
                .unwrap();
            assert!(
                parsed.diagnostics().is_empty(),
                "{:?}",
                parsed.diagnostics()
            );
            (path, document, parsed)
        })
        .collect::<Vec<(CanonicalModulePath, Arc<SourceDocument>, ParsedSource)>>();
    let root_document = modules
        .iter()
        .find(|(path, _, _)| path == &root)
        .map(|(_, document, _)| Arc::clone(document))
        .unwrap();
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        root_document.identity().id().clone(),
        "staged-project-test",
    )
    .unwrap();
    let revision = ProjectSymbolRevision::try_for_documents(
        modules.iter().map(|(_, document, _)| document.identity()),
    )
    .unwrap();
    let facts = ProjectRegistrationFacts::try_new(
        world.clone(),
        modules
            .iter()
            .map(|(_, document, _)| Arc::clone(document))
            .collect(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(facts.symbol_revision(), &revision);
    let mut database = HirDatabase::try_new().unwrap();
    let transaction = database
        .stage_proof_return_project(
            modules.iter().map(|(path, document, parsed)| {
                LoweringRequest::try_new(
                    HirModuleKey::new(package.clone(), path.clone(), document.identity().clone()),
                    parsed,
                )
                .unwrap()
            }),
            world,
            revision,
            facts.documents().map(|document| document.identity()),
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .unwrap();
    let generation = Arc::clone(transaction.generation());
    let headers = transaction.headers().cloned().collect::<Vec<_>>();
    let header_view = transaction.header_view();
    assert_eq!(headers.len(), 2);
    assert_eq!(header_view.modules().len(), 2);
    assert_eq!(header_view.authored_proof_returns().count(), 2);

    let prelude =
        CharacterRegistrar::prepare_proof_return_headers(ProofReturnRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            Arc::clone(&generation),
            header_view,
            &facts,
            None,
        ))
        .unwrap();
    let classification = classify_proof_return_project(
        generation,
        &headers,
        header_view,
        prelude.symbols(),
        prelude.nominal_world(),
    )
    .unwrap();
    assert_eq!(classification.reports().len(), 2);
    for header in &headers {
        assert_eq!(
            classification.facts().class_for(header).unwrap(),
            HirProofReturnSemanticClass::Unit
        );
    }

    let published = transaction
        .publish_with_semantic_facts(&mut database, classification.into_facts())
        .unwrap();
    assert_eq!(published.len(), 2);
    for output in published {
        assert!(Arc::ptr_eq(
            output.module(),
            &database.current(output.module().key()).unwrap()
        ));
    }
}
