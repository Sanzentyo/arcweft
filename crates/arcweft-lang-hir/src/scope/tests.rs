use super::*;
use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::identity::{HirDatabaseId, HirTypedId, RawHirId};

fn module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(slot).unwrap(),
    )
}

fn id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(Box::<str>::from(value)).unwrap()
}

fn source_span() -> arcweft_source::SourceSpan {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://hir/capture.arcw").unwrap(),
            SourceName::path("hir/capture.arcw"),
            "value",
        )
        .unwrap(),
    )
    .span(SourceRange::new(0, 5))
    .unwrap()
}

#[test]
fn scope_preserves_source_order_and_rejects_foreign_or_duplicate_children() {
    let module = module(1, 1);
    let foreign = self::module(2, 1);
    let parent = id::<ScopeId>(module, 1);
    let child_a = id::<ScopeId>(module, 2);
    let child_b = id::<ScopeId>(module, 3);
    let local = id::<LocalId>(module, 4);

    let scope = HirScope::try_new(
        module,
        HirScopeKind::Block,
        Some(parent),
        HirScopeOwner::Expr(id(module, 5)),
        Box::new([child_b, child_a]),
        Box::new([local]),
    )
    .unwrap();
    assert_eq!(scope.parent(), Some(parent));
    assert_eq!(scope.children(), [child_b, child_a]);
    assert_eq!(scope.locals(), [local]);

    assert_eq!(
        HirScope::try_new(
            module,
            HirScopeKind::Block,
            Some(parent),
            HirScopeOwner::Module(module),
            Box::new([child_a, child_a]),
            Box::new([]),
        ),
        Err(HirScopeInvariantError::DuplicateChild {
            kind: HirScopeChildKind::Scope,
        })
    );
    assert!(matches!(
        HirScope::try_new(
            module,
            HirScopeKind::Block,
            Some(id::<ScopeId>(foreign, 1)),
            HirScopeOwner::Module(module),
            Box::new([]),
            Box::new([]),
        ),
        Err(HirScopeInvariantError::ForeignReference { .. })
    ));
}

#[test]
fn every_scope_owner_is_semantic_and_module_qualified() {
    let module = module(5, 1);
    let owners = [
        HirScopeOwner::Module(module),
        HirScopeOwner::Item(id(module, 1)),
        HirScopeOwner::Expr(id(module, 2)),
        HirScopeOwner::Stmt(id(module, 3)),
    ];

    for owner in owners {
        let scope = HirScope::try_new(
            module,
            HirScopeKind::MatchArm,
            None,
            owner,
            Box::new([]),
            Box::new([]),
        )
        .unwrap();
        assert_eq!(scope.owner(), &owner);
    }

    assert!(matches!(
        HirScope::try_new(
            module,
            HirScopeKind::MatchArm,
            None,
            HirScopeOwner::Stmt(id(self::module(6, 1), 1)),
            Box::new([]),
            Box::new([]),
        ),
        Err(HirScopeInvariantError::ForeignReference { .. })
    ));
}

#[test]
fn closed_scope_kinds_admit_only_their_semantic_owner_families() {
    let module = module(7, 1);
    let item = HirScopeOwner::Item(id(module, 1));
    let expression = HirScopeOwner::Expr(id(module, 2));
    let statement = HirScopeOwner::Stmt(id(module, 3));
    let module_owner = HirScopeOwner::Module(module);
    let parent = id(module, 4);

    for (kind, owner) in [
        (HirScopeKind::Module, module_owner),
        (HirScopeKind::Callable, item),
        (HirScopeKind::Flow, item),
        (HirScopeKind::Predicate, item),
        (HirScopeKind::Proof, item),
        (HirScopeKind::Block, item),
        (HirScopeKind::Block, expression),
        (HirScopeKind::Block, statement),
        (HirScopeKind::MatchArm, expression),
        (HirScopeKind::MatchArm, statement),
        (HirScopeKind::Loop, statement),
        (HirScopeKind::Conditional, expression),
        (HirScopeKind::Conditional, statement),
        (HirScopeKind::Closure, expression),
        (HirScopeKind::ContractRequires, item),
        (HirScopeKind::ContractEnsures, item),
    ] {
        let scope = HirScope::try_new(
            module,
            kind,
            (kind != HirScopeKind::Module).then_some(parent),
            owner,
            Box::new([]),
            Box::new([]),
        )
        .unwrap();
        assert!(scope.has_admitted_owner(), "{kind:?} rejected {owner:?}");
    }

    for (kind, owner) in [
        (HirScopeKind::Module, item),
        (HirScopeKind::Callable, expression),
        (HirScopeKind::Flow, statement),
        (HirScopeKind::Predicate, module_owner),
        (HirScopeKind::Proof, expression),
        (HirScopeKind::MatchArm, item),
        (HirScopeKind::Loop, expression),
        (HirScopeKind::Conditional, item),
        (HirScopeKind::Closure, statement),
        (HirScopeKind::ContractRequires, expression),
        (HirScopeKind::ContractEnsures, statement),
    ] {
        let scope = HirScope::try_new(
            module,
            kind,
            Some(parent),
            owner,
            Box::new([]),
            Box::new([]),
        )
        .unwrap();
        assert!(
            !scope.has_admitted_owner(),
            "{kind:?} admitted invalid {owner:?}"
        );
    }
}

#[test]
fn local_and_capture_retain_typed_same_module_ownership() {
    let module = module(3, 1);
    let foreign = self::module(4, 1);
    let scope = id::<ScopeId>(module, 1);
    let pattern = id::<PatternId>(module, 2);
    let annotation = id::<TypeId>(module, 3);
    let local_id = id::<LocalId>(module, 4);
    let closure = id::<ExprId>(module, 5);

    let local = HirLocal::try_new(
        scope,
        HirLocalKind::PatternBinding,
        name("value"),
        LocalGeneration::FIRST,
        Some(pattern),
        Some(annotation),
        true,
        false,
    )
    .unwrap();
    assert_eq!(local.scope(), scope);
    assert_eq!(local.name().as_str(), "value");
    assert_eq!(local.generation().get(), 1);
    assert_eq!(local.pattern(), Some(pattern));
    assert_eq!(local.annotation(), Some(annotation));
    assert!(local.is_mutable_binding());
    assert!(!local.is_poisoned());

    let first_use = source_span();
    let capture = HirCapture::try_new(
        closure,
        local_id,
        CaptureAccess::Reassign,
        first_use.clone(),
    )
    .unwrap();
    assert_eq!(capture.closure(), closure);
    assert_eq!(capture.local(), local_id);
    assert_eq!(capture.access(), CaptureAccess::Reassign);
    assert_eq!(capture.first_use(), &first_use);

    assert!(matches!(
        HirCapture::try_new(
            closure,
            id::<LocalId>(foreign, 2),
            CaptureAccess::Read,
            source_span(),
        ),
        Err(HirScopeInvariantError::ForeignReference { .. })
    ));

    assert!(matches!(
        HirLocal::try_new(
            scope,
            HirLocalKind::LetBinding,
            name("foreign"),
            LocalGeneration::FIRST,
            Some(id::<PatternId>(foreign, 1)),
            None,
            false,
            true,
        ),
        Err(HirScopeInvariantError::ForeignReference { .. })
    ));
}

#[test]
fn local_generations_are_nonzero_monotonic_and_nonwrapping() {
    assert!(LocalGeneration::try_new(0).is_none());
    assert_eq!(LocalGeneration::FIRST.checked_next().unwrap().get(), 2);
    assert!(
        LocalGeneration::try_new(u32::MAX)
            .unwrap()
            .checked_next()
            .is_none()
    );
}
