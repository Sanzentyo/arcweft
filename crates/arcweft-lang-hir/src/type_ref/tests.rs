use core::num::{NonZeroU32, NonZeroU64};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    HirAssociatedTypeBinding, HirFunctionType, HirGenericType, HirGenericTypeIssue,
    HirProjectionType, HirReferenceType, HirTraitBoundType, HirType, HirTypeEffectRow,
    HirTypeError, HirTypeInvariantError, HirTypeKind, HirTypeResolver,
};
use crate::callable_source::HirEffectName;
use crate::expr::{HirBorrowKind, HirPoisonState, HirRecoveryIssue};
use crate::identity::{
    HirDatabaseId, HirModuleId, HirTypedId, RawHirId, ScopeId, SyntheticKey, SyntheticOwner,
    SyntheticRole, TypeId,
};
use crate::leaf::{
    HirElidedRegion, HirName, HirPath, HirPathRoot, HirPathSegment, HirRegionName, HirTypeRegion,
    HirTypeRegionIssue,
};

#[derive(Default)]
struct TestResolver {
    scopes: BTreeSet<ScopeId>,
    types: BTreeMap<(ScopeId, TypeId), HirType>,
}

impl TestResolver {
    fn with_scope(scope: ScopeId) -> Self {
        Self {
            scopes: BTreeSet::from([scope]),
            ..Self::default()
        }
    }

    fn admit_type(&mut self, scope: ScopeId, id: TypeId, ty: HirType) {
        self.types.insert((scope, id), ty);
    }
}

impl HirTypeResolver for TestResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        self.scopes.contains(&scope)
    }

    fn resolve_type(&self, scope: ScopeId, ty: TypeId) -> Option<&HirType> {
        self.types.get(&(scope, ty))
    }
}

fn test_module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).expect("nonzero database")),
        NonZeroU32::new(slot).expect("nonzero module slot"),
    )
}

fn id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).expect("valid test name")
}

fn path(root: HirPathRoot, value: &str) -> HirPath {
    HirPath::try_new(
        root,
        vec![HirPathSegment::Identifier(name(value))].into_boxed_slice(),
    )
    .expect("nonempty test path")
}

fn clean(
    resolver: &TestResolver,
    owner: TypeId,
    scope: ScopeId,
    kind: HirTypeKind,
) -> Result<HirType, HirTypeInvariantError> {
    HirType::try_new(owner, kind, scope, HirPoisonState::Clean, resolver)
}

fn poisoned(
    resolver: &TestResolver,
    owner: TypeId,
    scope: ScopeId,
    kind: HirTypeKind,
) -> Result<HirType, HirTypeInvariantError> {
    let issue = match &kind {
        HirTypeKind::Recovery(error) => HirRecoveryIssue::InvalidType(error.issue()),
        _ => HirRecoveryIssue::MissingRequiredTail,
    };
    HirType::try_new(
        owner,
        kind,
        scope,
        HirPoisonState::Poisoned(issue),
        resolver,
    )
}

fn named_region(value: &str) -> HirTypeRegion {
    HirTypeRegion::named(HirRegionName::new(name(value)))
}

fn elided_region(owner: TypeId) -> HirTypeRegion {
    let key = SyntheticKey::try_new(SyntheticOwner::Type(owner), SyntheticRole::ElidedRegion, 0)
        .expect("valid elided-region key");
    HirTypeRegion::elided(HirElidedRegion::try_new(owner, key).expect("matching type owner"))
}

#[test]
fn exact_twelve_type_families_construct_through_one_typed_owner() {
    let module = test_module(1, 1);
    let scope = id::<ScopeId>(module, 1);
    let child_a = id::<TypeId>(module, 2);
    let child_b = id::<TypeId>(module, 3);
    let mut resolver = TestResolver::with_scope(scope);
    let first = clean(&resolver, child_a, scope, HirTypeKind::Never).expect("first child");
    resolver.admit_type(scope, child_a, first);
    let second = clean(&resolver, child_b, scope, HirTypeKind::ConstInt(3)).expect("second child");
    resolver.admit_type(scope, child_b, second);

    let effects = HirTypeEffectRow::new(vec![
        HirEffectName::try_new("state.read('flow)").expect("valid effect"),
        HirEffectName::try_new("fs.read").expect("valid effect"),
    ]);
    let owners = (10..22)
        .map(|slot| id::<TypeId>(module, slot))
        .collect::<Vec<_>>();
    let families = vec![
        HirTypeKind::Never,
        HirTypeKind::ConstInt(usize::MAX),
        HirTypeKind::Path(path(HirPathRoot::Crate, "Value")),
        HirTypeKind::Tuple(vec![child_a, child_b].into_boxed_slice()),
        HirTypeKind::Function(HirFunctionType::new(
            vec![child_a, child_b].into_boxed_slice(),
            child_a,
            Some(effects),
        )),
        HirTypeKind::Choice(vec![child_a, child_b].into_boxed_slice()),
        HirTypeKind::Generic(HirGenericType::new(
            path(HirPathRoot::SelfModule, "Vec"),
            vec![child_a].into_boxed_slice(),
        )),
        HirTypeKind::TraitBound(HirTraitBoundType::new(
            path(HirPathRoot::ImplicitCrate, "Iterator"),
            vec![child_a].into_boxed_slice(),
            vec![HirAssociatedTypeBinding::new(name("Item"), child_b)].into_boxed_slice(),
        )),
        HirTypeKind::Projection(HirProjectionType::new(child_a, name("Output"))),
        HirTypeKind::Reference(HirReferenceType::new(
            HirBorrowKind::Mutable,
            Some(elided_region(owners[9])),
            child_a,
        )),
        HirTypeKind::Slice(child_b),
        HirTypeKind::Recovery(HirTypeError::new(HirGenericTypeIssue::UnclassifiedSyntax)),
    ];

    assert_eq!(families.len(), 12);
    for (ordinal, (owner, kind)) in owners.into_iter().zip(families).enumerate() {
        let ty = if ordinal == 11 {
            poisoned(&resolver, owner, scope, kind)
        } else {
            clean(&resolver, owner, scope, kind)
        }
        .expect("the final type family should construct");
        assert_eq!(ty.scope(), scope);
    }
}

#[test]
fn function_effects_and_trait_bindings_preserve_authored_order_and_typed_values() {
    let module = test_module(2, 1);
    let scope = id::<ScopeId>(module, 1);
    let first_id = id::<TypeId>(module, 2);
    let second_id = id::<TypeId>(module, 3);
    let function_id = id::<TypeId>(module, 4);
    let trait_id = id::<TypeId>(module, 5);
    let mut resolver = TestResolver::with_scope(scope);
    let first = clean(&resolver, first_id, scope, HirTypeKind::Never).expect("first child");
    resolver.admit_type(scope, first_id, first);
    let second =
        clean(&resolver, second_id, scope, HirTypeKind::ConstInt(9)).expect("second child");
    resolver.admit_type(scope, second_id, second);

    let function = clean(
        &resolver,
        function_id,
        scope,
        HirTypeKind::Function(HirFunctionType::new(
            vec![second_id, first_id].into_boxed_slice(),
            second_id,
            Some(HirTypeEffectRow::new(vec![
                HirEffectName::try_new("state.write('flow)").expect("valid effect"),
                HirEffectName::try_new("audio.play").expect("valid effect"),
            ])),
        )),
    )
    .expect("function type");
    let HirTypeKind::Function(function) = function.kind() else {
        panic!("function family")
    };
    assert_eq!(function.parameters(), [second_id, first_id]);
    assert_eq!(function.return_type(), second_id);
    let effect_labels = function
        .effects()
        .expect("present effect row")
        .effects()
        .iter()
        .map(HirEffectName::as_str)
        .collect::<Vec<_>>();
    assert_eq!(effect_labels, ["state.write('flow)", "audio.play"]);

    let bound = clean(
        &resolver,
        trait_id,
        scope,
        HirTypeKind::TraitBound(HirTraitBoundType::new(
            path(HirPathRoot::Super { depth: 2 }, "Iterator"),
            vec![second_id, first_id].into_boxed_slice(),
            vec![
                HirAssociatedTypeBinding::new(name("Item"), first_id),
                HirAssociatedTypeBinding::new(name("Error"), second_id),
            ]
            .into_boxed_slice(),
        )),
    )
    .expect("trait bound");
    let HirTypeKind::TraitBound(bound) = bound.kind() else {
        panic!("trait-bound family")
    };
    assert_eq!(bound.base().root(), HirPathRoot::Super { depth: 2 });
    assert_eq!(bound.arguments(), [second_id, first_id]);
    assert_eq!(bound.associated()[0].name().as_str(), "Item");
    assert_eq!(bound.associated()[0].value(), first_id);
    assert_eq!(bound.associated()[1].name().as_str(), "Error");
    assert_eq!(bound.associated()[1].value(), second_id);
}

#[test]
fn every_child_type_requires_same_module_transaction_visibility() {
    let module = test_module(3, 1);
    let foreign_module = test_module(4, 1);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<TypeId>(module, 2);
    let hidden = id::<TypeId>(module, 3);
    let foreign = id::<TypeId>(foreign_module, 1);
    let resolver = TestResolver::with_scope(scope);

    assert_eq!(
        clean(
            &resolver,
            owner,
            scope,
            HirTypeKind::Tuple(vec![hidden].into_boxed_slice()),
        ),
        Err(HirTypeInvariantError::TypeNotVisible { scope, ty: hidden })
    );
    assert_eq!(
        clean(
            &resolver,
            owner,
            scope,
            HirTypeKind::Projection(HirProjectionType::new(foreign, name("Item"))),
        ),
        Err(HirTypeInvariantError::ForeignType {
            expected: module,
            actual: foreign_module,
        })
    );

    let foreign_scope = id::<ScopeId>(foreign_module, 2);
    assert_eq!(
        clean(&resolver, owner, foreign_scope, HirTypeKind::Never),
        Err(HirTypeInvariantError::ForeignScope {
            expected: module,
            actual: foreign_module,
        })
    );
    let dead_scope = id::<ScopeId>(module, 9);
    assert_eq!(
        clean(&resolver, owner, dead_scope, HirTypeKind::Never),
        Err(HirTypeInvariantError::ScopeNotLive { scope: dead_scope })
    );
}

#[test]
fn reference_types_accept_only_their_own_elided_region_identity() {
    let module = test_module(5, 1);
    let scope = id::<ScopeId>(module, 1);
    let referent_id = id::<TypeId>(module, 2);
    let owner = id::<TypeId>(module, 3);
    let other = id::<TypeId>(module, 4);
    let mut resolver = TestResolver::with_scope(scope);
    let referent = clean(&resolver, referent_id, scope, HirTypeKind::Never).expect("referent");
    resolver.admit_type(scope, referent_id, referent);

    let named = clean(
        &resolver,
        owner,
        scope,
        HirTypeKind::Reference(HirReferenceType::new(
            HirBorrowKind::Shared,
            Some(named_region("scene")),
            referent_id,
        )),
    )
    .expect("named region");
    let HirTypeKind::Reference(named) = named.kind() else {
        panic!("reference family")
    };
    assert!(matches!(named.region(), Some(HirTypeRegion::Named(_))));

    assert_eq!(
        clean(
            &resolver,
            owner,
            scope,
            HirTypeKind::Reference(HirReferenceType::new(
                HirBorrowKind::Mutable,
                Some(elided_region(other)),
                referent_id,
            )),
        ),
        Err(HirTypeInvariantError::ElidedRegionOwnerMismatch {
            expected: owner,
            actual: other,
        })
    );
    let elided = clean(
        &resolver,
        owner,
        scope,
        HirTypeKind::Reference(HirReferenceType::new(
            HirBorrowKind::Mutable,
            Some(elided_region(owner)),
            referent_id,
        )),
    )
    .expect("owner-bound elided region");
    let HirTypeKind::Reference(elided) = elided.kind() else {
        panic!("reference family")
    };
    assert_eq!(elided.kind(), HirBorrowKind::Mutable);
    assert_eq!(elided.referent(), referent_id);
    assert!(matches!(elided.region(), Some(HirTypeRegion::Elided(_))));
}

#[test]
fn missing_reference_region_requires_exact_invalid_named_region_poison() {
    let module = test_module(8, 1);
    let scope = id::<ScopeId>(module, 1);
    let referent_id = id::<TypeId>(module, 2);
    let owner = id::<TypeId>(module, 3);
    let mut resolver = TestResolver::with_scope(scope);
    let referent = clean(&resolver, referent_id, scope, HirTypeKind::Never).expect("referent");
    resolver.admit_type(scope, referent_id, referent);

    let missing_region = || {
        HirTypeKind::Reference(HirReferenceType::new(
            HirBorrowKind::Shared,
            None,
            referent_id,
        ))
    };
    assert_eq!(
        clean(&resolver, owner, scope, missing_region()),
        Err(HirTypeInvariantError::MissingReferenceRegionRequiresInvalidNamedRegionPoison)
    );
    assert_eq!(
        HirType::try_new(
            owner,
            missing_region(),
            scope,
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
            &resolver,
        ),
        Err(HirTypeInvariantError::MissingReferenceRegionRequiresInvalidNamedRegionPoison)
    );

    let invalid_named_region =
        HirRecoveryIssue::InvalidTypeRegion(HirTypeRegionIssue::InvalidNamedRegion);
    let recovered = HirType::try_new(
        owner,
        missing_region(),
        scope,
        HirPoisonState::Poisoned(invalid_named_region.clone()),
        &resolver,
    )
    .expect("known reference family retains the referent while its named region is poisoned");
    let HirTypeKind::Reference(reference) = recovered.kind() else {
        panic!("reference family")
    };
    assert_eq!(reference.region(), None);
    assert_eq!(reference.referent(), referent_id);

    assert_eq!(
        HirType::try_new(
            owner,
            HirTypeKind::Reference(HirReferenceType::new(
                HirBorrowKind::Shared,
                Some(named_region("scene")),
                referent_id,
            )),
            scope,
            HirPoisonState::Poisoned(invalid_named_region.clone()),
            &resolver,
        ),
        Err(HirTypeInvariantError::InvalidNamedRegionPoisonRequiresMissingReferenceRegion)
    );
    assert_eq!(
        HirType::try_new(
            owner,
            HirTypeKind::Never,
            scope,
            HirPoisonState::Poisoned(invalid_named_region),
            &resolver,
        ),
        Err(HirTypeInvariantError::InvalidNamedRegionPoisonRequiresMissingReferenceRegion)
    );
}

#[test]
fn clean_types_reject_generic_recovery_while_poisoned_types_retain_it() {
    let module = test_module(6, 1);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<TypeId>(module, 2);
    let resolver = TestResolver::with_scope(scope);
    let recovery = HirTypeKind::Recovery(HirTypeError::new(
        HirGenericTypeIssue::TransactionalChildFailure,
    ));

    assert_eq!(
        clean(&resolver, owner, scope, recovery.clone()),
        Err(HirTypeInvariantError::CleanRecoveryPayload)
    );
    let retained = poisoned(&resolver, owner, scope, recovery).expect("poisoned recovery");
    let HirTypeKind::Recovery(error) = retained.kind() else {
        panic!("recovery family")
    };
    assert_eq!(
        error.issue(),
        HirGenericTypeIssue::TransactionalChildFailure
    );
    assert!(matches!(retained.state(), HirPoisonState::Poisoned(_)));

    assert_eq!(
        HirType::try_new(
            owner,
            HirTypeKind::Recovery(HirTypeError::new(HirGenericTypeIssue::UnclassifiedSyntax,)),
            scope,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidType(
                HirGenericTypeIssue::TransactionalChildFailure,
            )),
            &resolver,
        ),
        Err(HirTypeInvariantError::RecoveryIssueMismatch)
    );
    assert_eq!(
        HirType::try_new(
            owner,
            HirTypeKind::Never,
            scope,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidType(
                HirGenericTypeIssue::UnclassifiedSyntax,
            )),
            &resolver,
        ),
        Err(HirTypeInvariantError::UnexpectedGenericRecoveryIssue)
    );
}

#[test]
fn path_roots_and_absent_versus_empty_effect_rows_remain_distinct() {
    let roots = [
        HirPathRoot::ImplicitCrate,
        HirPathRoot::Crate,
        HirPathRoot::SelfModule,
        HirPathRoot::Super { depth: usize::MAX },
    ];
    for root in roots {
        assert_eq!(path(root, "Value").root(), root);
    }

    let module = test_module(7, 1);
    let scope = id::<ScopeId>(module, 1);
    let child_id = id::<TypeId>(module, 2);
    let absent_id = id::<TypeId>(module, 3);
    let empty_id = id::<TypeId>(module, 4);
    let mut resolver = TestResolver::with_scope(scope);
    let child = clean(&resolver, child_id, scope, HirTypeKind::Never).expect("child");
    resolver.admit_type(scope, child_id, child);

    let absent = clean(
        &resolver,
        absent_id,
        scope,
        HirTypeKind::Function(HirFunctionType::new(Box::new([]), child_id, None)),
    )
    .expect("absent row");
    let empty = clean(
        &resolver,
        empty_id,
        scope,
        HirTypeKind::Function(HirFunctionType::new(
            Box::new([]),
            child_id,
            Some(HirTypeEffectRow::new(Vec::new())),
        )),
    )
    .expect("empty row");
    let HirTypeKind::Function(absent) = absent.kind() else {
        panic!("function family")
    };
    let HirTypeKind::Function(empty) = empty.kind() else {
        panic!("function family")
    };
    assert!(absent.effects().is_none());
    assert_eq!(empty.effects().expect("present empty row").effects(), []);
}
