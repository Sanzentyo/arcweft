use std::sync::Arc;

use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{HirProject, HirProjectModule};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

#[test]
fn public_final_hir_boundary_preserves_the_accepted_module_lease() {
    let package = CallablePackageId::try_new("proof-final-hir-public-api").unwrap();
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path("final-hir-public-api.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://proof/final-hir-public-api").unwrap(),
            source_name.clone(),
            "fn root() {}\n",
        )
        .unwrap(),
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();

    let key = HirModuleKey::new(
        package.clone(),
        path.clone(),
        parsed.document().identity().clone(),
    );
    let mut database = HirDatabase::try_new().unwrap();
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        parsed.document().identity().id().clone(),
        "public-api-test",
    )
    .unwrap();
    let revision =
        ProjectSymbolRevision::try_for_documents([parsed.document().identity()]).unwrap();
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(key, &parsed).unwrap()],
            world,
            revision,
            [parsed.document().identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .unwrap();
    let facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("fixture has no authored Proof returns");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut database, facts)
        .unwrap();
    let module = outputs.pop().expect("one published module").into_module();
    assert!(outputs.is_empty());
    let project_module = HirProjectModule::try_new(
        &database,
        &package,
        &path,
        parsed.document().identity(),
        Arc::clone(&module),
    )
    .unwrap();
    let project = HirProject::try_new(&database, package, [project_module]).unwrap();

    let retained = project.module(&path).unwrap().module();
    assert!(Arc::ptr_eq(retained, &module));
    let item = project.view().items().next().unwrap();
    assert!(Arc::ptr_eq(item.module(), &module));
    assert!(std::ptr::eq(
        item.item(),
        module.resolve_item(item.id()).unwrap(),
    ));
}
