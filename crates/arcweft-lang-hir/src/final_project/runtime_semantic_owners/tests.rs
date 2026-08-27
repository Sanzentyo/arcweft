use core::num::{NonZeroU32, NonZeroU64};

use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::*;
use crate::identity::{HirDatabaseId, HirIdKind, HirModuleId, HirTypedId, RawHirId};
use crate::symbol::{
    CallablePackageId, ImplDeclarationId, ImplMethodDeclarationId, ImplMethodKind,
};

fn statement(module: HirModuleId) -> StmtId {
    StmtId::from_raw(RawHirId::new(module, NonZeroU32::MIN, HirIdKind::Stmt))
}

fn item(module: HirModuleId, slot: u32) -> ItemId {
    ItemId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero item slot"),
        HirIdKind::Item,
    ))
}

fn method(package: &CallablePackageId, ordinal: u32, name: &str) -> ImplMethodDeclarationId {
    ImplMethodDeclarationId::new(
        ImplDeclarationId::new(package.clone(), CanonicalModulePath::crate_root(), ordinal),
        ImplMethodKind::Trait,
        ModuleSegment::new(name).expect("valid test method name"),
    )
}

fn generation() -> (ProjectSymbolWorldId, ProjectSymbolRevision) {
    let package =
        CallablePackageId::try_new("runtime-iterator-edge-tests").expect("valid test package");
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://runtime-iterator-edge")
            .expect("valid test document ID"),
        SourceName::Generated,
        "",
    )
    .expect("test document");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "runtime-iterator-edge",
    )
    .expect("test symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([document.identity()])
        .expect("test symbol revision");
    (world, revision)
}

#[test]
fn iterator_witness_rejects_two_methods_for_one_statement_role() {
    let module = HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::MIN),
        NonZeroU32::MIN,
    );
    let statement = statement(module);
    let package =
        CallablePackageId::try_new("runtime-iterator-edge-tests").expect("valid test package");
    let first_method = method(&package, 0, "first");
    let second_method = method(&package, 1, "second");
    let edge = |implementation: ItemId, method: ImplMethodDeclarationId| {
        HirRuntimeReachabilityEdge::new(
            HirRuntimeReachabilitySite::Statement(statement),
            HirRuntimeExecutableOwner::ImplMethod(method.clone()),
            HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
                role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
                implementation,
                member: 0,
                method,
            },
        )
    };
    let (world, revision) = generation();
    assert!(matches!(
        HirRuntimeSemanticReachabilityInput::try_new(
            HirRuntimeEmissionMode::CheckAll,
            world,
            revision,
            Vec::new(),
            vec![
                edge(item(module, 2), first_method),
                edge(item(module, 3), second_method),
            ],
        ),
        Err(HirRuntimeReachabilityError::DuplicateIteratorWitnessMethodRole {
            site: HirRuntimeReachabilitySite::Statement(found),
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }) if found == statement
    ));
}
