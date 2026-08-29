use super::{
    CaptureId, ExprId, HirDatabaseCreateError, HirDatabaseId, HirIdKind, HirLimit, HirModuleId,
    HirRevision, HirSnapshotId, HirTypedId, IdResolveError, ItemId, LocalId, PatternId, RawHirId,
    RawHirIdView, ScopeId, StmtId, SyntheticKey, SyntheticKeyError, SyntheticKeyFingerprintInput,
    SyntheticOwner, SyntheticRole, TypeId, allocate_database_id,
};
use core::fmt::Debug;
use core::hash::Hash;
use core::num::{NonZeroU32, NonZeroU64};
use core::sync::atomic::AtomicU64;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Write;

fn module_id(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(slot).unwrap(),
    )
}

fn expression(module: HirModuleId, slot: u32) -> ExprId {
    ExprId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        HirIdKind::Expr,
    ))
}

fn raw_id(module: HirModuleId, slot: u32, kind: HirIdKind) -> RawHirId {
    RawHirId::new(module, NonZeroU32::new(slot).unwrap(), kind)
}

fn owner(module: HirModuleId, slot: u32, kind: HirIdKind) -> SyntheticOwner {
    let raw = raw_id(module, slot, kind);
    match kind {
        HirIdKind::Item => SyntheticOwner::Item(ItemId::from_raw(raw)),
        HirIdKind::Scope => SyntheticOwner::Scope(ScopeId::from_raw(raw)),
        HirIdKind::Local => SyntheticOwner::Local(LocalId::from_raw(raw)),
        HirIdKind::Expr => SyntheticOwner::Expr(ExprId::from_raw(raw)),
        HirIdKind::Stmt => SyntheticOwner::Stmt(StmtId::from_raw(raw)),
        HirIdKind::Type => SyntheticOwner::Type(TypeId::from_raw(raw)),
        HirIdKind::Pattern => SyntheticOwner::Pattern(PatternId::from_raw(raw)),
        HirIdKind::Capture => SyntheticOwner::Capture(CaptureId::from_raw(raw)),
    }
}

fn fingerprint_hex(input: SyntheticKeyFingerprintInput) -> String {
    let mut result = String::with_capacity(input.as_bytes().len() * 2);
    for byte in input.as_bytes() {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

#[test]
fn database_ids_are_nonzero_and_never_wrap() {
    let counter = AtomicU64::new(7);
    let first = allocate_database_id(&counter).unwrap();
    let second = allocate_database_id(&counter).unwrap();
    assert!(first < second);

    let boundary = AtomicU64::new(u64::MAX - 1);
    assert_eq!(
        allocate_database_id(&boundary),
        Some(HirDatabaseId::from_raw_for_test(
            NonZeroU64::new(u64::MAX - 1).unwrap()
        ))
    );
    assert_eq!(
        allocate_database_id(&boundary),
        Some(HirDatabaseId::from_raw_for_test(
            NonZeroU64::new(u64::MAX).unwrap()
        ))
    );
    assert_eq!(allocate_database_id(&boundary), None);
    assert_eq!(boundary.load(core::sync::atomic::Ordering::Relaxed), 0);
    let exhausted = AtomicU64::new(0);
    assert_eq!(allocate_database_id(&exhausted), None);

    assert_eq!(
        HirDatabaseCreateError::IdentityExhausted.to_string(),
        "HIR database identity allocation is exhausted"
    );
}

#[test]
fn revisions_start_at_one_and_never_wrap() {
    let first = HirRevision::INITIAL;
    let second = first.checked_next().unwrap();
    assert!(second > first);

    let exhausted = HirRevision::from_raw_for_test(NonZeroU32::new(u32::MAX).unwrap());
    assert_eq!(exhausted.checked_next(), None);
}

#[test]
fn exactly_eight_typed_id_kinds_share_the_sealed_raw_identity() {
    fn round_trip<I: HirTypedId>(module: HirModuleId, slot: u32) -> RawHirId {
        let raw = RawHirId::new(module, NonZeroU32::new(slot).unwrap(), I::KIND);
        I::from_raw(raw).raw()
    }

    let module = module_id(41, 43);
    let rows = [
        round_trip::<ItemId>(module, 1),
        round_trip::<ScopeId>(module, 2),
        round_trip::<LocalId>(module, 3),
        round_trip::<ExprId>(module, 4),
        round_trip::<StmtId>(module, 5),
        round_trip::<TypeId>(module, 6),
        round_trip::<PatternId>(module, 7),
        round_trip::<CaptureId>(module, 8),
    ];
    assert_eq!(rows.map(RawHirId::kind), ALL_KINDS);
    assert!(rows.iter().all(|id| id.module() == module));
    assert_eq!(rows[0].slot(), NonZeroU32::MIN);
}

#[test]
fn typed_ids_include_database_module_kind_and_global_slot() {
    let module = module_id(1, 2);
    let first = expression(module, 3);
    let second = expression(module, 4);
    let foreign_database = expression(module_id(2, 2), 3);

    assert!(first < second);
    assert!(first < foreign_database);
    assert_ne!(first, foreign_database);
    assert_eq!(first.module(), module);
    assert_eq!(first.kind(), HirIdKind::Expr);

    let snapshot = HirSnapshotId {
        module,
        revision: HirRevision(NonZeroU32::MIN),
    };
    assert_eq!(snapshot.module(), module);
    assert_eq!(snapshot.revision().0.get(), 1);
}

#[test]
fn id_resolve_error_variants_preserve_exact_payload_shapes() {
    let module = module_id(7, 11);
    let id = RawHirIdView::from(RawHirId {
        module,
        slot: NonZeroU32::new(13).unwrap(),
        kind: HirIdKind::Expr,
    });
    let snapshot = HirSnapshotId {
        module,
        revision: HirRevision(NonZeroU32::new(3).unwrap()),
    };

    assert_eq!(id.module(), module);
    assert_eq!(id.kind(), HirIdKind::Expr);
    assert_eq!(id.slot.get(), 13);

    let corrupted_wrapper = ExprId(RawHirId {
        module,
        slot: NonZeroU32::new(14).unwrap(),
        kind: HirIdKind::Stmt,
    });
    assert_eq!(corrupted_wrapper.kind(), HirIdKind::Expr);
    assert_eq!(
        RawHirIdView::from(corrupted_wrapper.0).kind(),
        HirIdKind::Stmt
    );

    match (IdResolveError::WrongModule {
        expected: module,
        actual: module_id(8, 11),
    }) {
        IdResolveError::WrongModule { expected, actual } => {
            assert_eq!(expected, module);
            assert_eq!(actual, module_id(8, 11));
        }
        other => panic!("unexpected resolver error: {other:?}"),
    }

    match (IdResolveError::NotYetLive {
        id,
        snapshot,
        born: HirRevision(NonZeroU32::new(4).unwrap()),
    }) {
        IdResolveError::NotYetLive {
            id: actual_id,
            snapshot: actual_snapshot,
            born,
        } => {
            assert_eq!(actual_id, id);
            assert_eq!(actual_snapshot, snapshot);
            assert_eq!(born.0.get(), 4);
        }
        other => panic!("unexpected resolver error: {other:?}"),
    }

    match (IdResolveError::Retired {
        id,
        snapshot,
        retired_at: HirRevision(NonZeroU32::new(3).unwrap()),
    }) {
        IdResolveError::Retired {
            id: actual_id,
            snapshot: actual_snapshot,
            retired_at,
        } => {
            assert_eq!(actual_id, id);
            assert_eq!(actual_snapshot, snapshot);
            assert_eq!(retired_at.0.get(), 3);
        }
        other => panic!("unexpected resolver error: {other:?}"),
    }

    match (IdResolveError::KindMismatch {
        id,
        expected: HirIdKind::Expr,
        actual: HirIdKind::Stmt,
    }) {
        IdResolveError::KindMismatch {
            id: actual_id,
            expected,
            actual,
        } => {
            assert_eq!(actual_id, id);
            assert_eq!(expected, HirIdKind::Expr);
            assert_eq!(actual, HirIdKind::Stmt);
        }
        other => panic!("unexpected resolver error: {other:?}"),
    }
}

#[test]
fn synthetic_owner_projects_every_typed_id_family() {
    fn assert_structural_traits<
        T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
    >() {
    }

    assert_structural_traits::<SyntheticOwner>();

    let module = module_id(17, 19);
    let owners = [
        (
            SyntheticOwner::Item(ItemId(raw_id(module, 1, HirIdKind::Item))),
            HirIdKind::Item,
        ),
        (
            SyntheticOwner::Scope(ScopeId(raw_id(module, 2, HirIdKind::Scope))),
            HirIdKind::Scope,
        ),
        (
            SyntheticOwner::Local(LocalId(raw_id(module, 3, HirIdKind::Local))),
            HirIdKind::Local,
        ),
        (
            SyntheticOwner::Expr(ExprId(raw_id(module, 4, HirIdKind::Expr))),
            HirIdKind::Expr,
        ),
        (
            SyntheticOwner::Stmt(StmtId(raw_id(module, 5, HirIdKind::Stmt))),
            HirIdKind::Stmt,
        ),
        (
            SyntheticOwner::Type(TypeId(raw_id(module, 6, HirIdKind::Type))),
            HirIdKind::Type,
        ),
        (
            SyntheticOwner::Pattern(PatternId(raw_id(module, 7, HirIdKind::Pattern))),
            HirIdKind::Pattern,
        ),
        (
            SyntheticOwner::Capture(CaptureId(raw_id(module, 8, HirIdKind::Capture))),
            HirIdKind::Capture,
        ),
    ];

    for (owner, expected_kind) in owners {
        assert_eq!(owner.kind(), expected_kind);
        assert_eq!(owner.module(), module);
    }

    let shared_raw = raw_id(module, 21, HirIdKind::Expr);
    let item = SyntheticOwner::Item(ItemId(shared_raw));
    let expression = SyntheticOwner::Expr(ExprId(shared_raw));
    assert_eq!(item.kind(), HirIdKind::Item);
    assert_eq!(expression.kind(), HirIdKind::Expr);
    assert_ne!(item, expression);
    assert!(item < expression);
}

#[test]
fn owned_identity_vocabularies_have_stable_behavior() {
    assert_eq!(HirIdKind::Capture.as_str(), "capture");
    assert_eq!(HirLimit::LocalsPerScope.maximum(), 4_096);
    assert_eq!(HirLimit::DeclarationMembers.maximum(), 1_024);
    assert_eq!(HirLimit::Captures.maximum(), 65_536);
    assert_eq!(HirLimit::TotalSlotsPerModule.maximum(), 786_432);
    assert_eq!(HirLimit::SourceDocumentBytes.maximum(), 8_388_608);
    assert_eq!(HirLimit::DecodedStringBytes.maximum(), 8_388_608);
    assert_eq!(HirLimit::NameBytes.maximum(), 1_024);
    assert_eq!(HirLimit::PathSegments.maximum(), 256);
    assert_eq!(HirLimit::PathSemanticBytes.maximum(), 65_536);
    assert_eq!(HirLimit::RegistrySegments.maximum(), 256);
    assert_eq!(HirLimit::RegistrySemanticBytes.maximum(), 65_536);
    assert_eq!(HirLimit::NumericDigitsPerLiteral.maximum(), 65_536);
    assert_eq!(HirLimit::DecimalCoefficientDigits.maximum(), 65_536);
    assert_eq!(HirLimit::DecimalScale.maximum(), 65_536);
    assert_eq!(HirLimit::DecimalExponentAbs.maximum(), 1_000_000);
    assert_eq!(HirLimit::NumericSequenceElements.maximum(), 65_536);
    assert_eq!(HirLimit::NumericSequenceTotalDigits.maximum(), 262_144);
    assert_eq!(HirLimit::ThreadFlowItems.maximum(), 65_536);
    assert_eq!(HirLimit::DialogueMarksPerContent.maximum(), 4_096);
    assert_eq!(HirLimit::SelectBranches.maximum(), 65_536);
    assert_eq!(HirLimit::StyleNestingDepth.maximum(), 64);
    assert_eq!(SyntheticRole::ElidedRegion.as_str(), "elided_region");
    assert_eq!(SyntheticRole::ClosureCapture.as_str(), "closure_capture");
    assert_eq!(
        SyntheticRole::ContractEnsuresScope.as_str(),
        "contract_ensures_scope"
    );
    assert_eq!(
        SyntheticRole::PostfixIndexCandidateExpression.as_str(),
        "postfix_index_candidate_expression"
    );
    assert_eq!(
        SyntheticRole::DialogueContentCandidateExpression.as_str(),
        "dialogue_content_candidate_expression"
    );
}

#[derive(Clone, Copy)]
struct RoleCase {
    role: SyntheticRole,
    accepted_kinds: &'static [HirIdKind],
    source_ordered: bool,
    tag: u8,
}

const ALL_KINDS: [HirIdKind; 8] = [
    HirIdKind::Item,
    HirIdKind::Scope,
    HirIdKind::Local,
    HirIdKind::Expr,
    HirIdKind::Stmt,
    HirIdKind::Type,
    HirIdKind::Pattern,
    HirIdKind::Capture,
];

const ROLE_CASES: [RoleCase; 20] = [
    RoleCase {
        role: SyntheticRole::ImplicitUnitTail,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Scope],
        source_ordered: false,
        tag: 0x01,
    },
    RoleCase {
        role: SyntheticRole::PredicateBoolReturn,
        accepted_kinds: &[HirIdKind::Item],
        source_ordered: false,
        tag: 0x02,
    },
    RoleCase {
        role: SyntheticRole::ProofUnitReturn,
        accepted_kinds: &[HirIdKind::Item],
        source_ordered: false,
        tag: 0x03,
    },
    RoleCase {
        role: SyntheticRole::ElidedRegion,
        accepted_kinds: &[HirIdKind::Type],
        source_ordered: false,
        tag: 0x04,
    },
    RoleCase {
        role: SyntheticRole::RecoveryOperand,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Stmt],
        source_ordered: true,
        tag: 0x05,
    },
    RoleCase {
        role: SyntheticRole::PostconditionResult,
        accepted_kinds: &[HirIdKind::Scope],
        source_ordered: false,
        tag: 0x06,
    },
    RoleCase {
        role: SyntheticRole::DesugaredTemporary,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Stmt],
        source_ordered: true,
        tag: 0x07,
    },
    RoleCase {
        role: SyntheticRole::MissingRequiredTail,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Scope],
        source_ordered: false,
        tag: 0x08,
    },
    RoleCase {
        role: SyntheticRole::DestructuredBinding,
        accepted_kinds: &[HirIdKind::Pattern],
        source_ordered: true,
        tag: 0x09,
    },
    RoleCase {
        role: SyntheticRole::ClosureCapture,
        accepted_kinds: &[HirIdKind::Expr],
        source_ordered: true,
        tag: 0x0b,
    },
    RoleCase {
        role: SyntheticRole::ContractRequiresScope,
        accepted_kinds: &[HirIdKind::Item],
        source_ordered: false,
        tag: 0x0c,
    },
    RoleCase {
        role: SyntheticRole::ContractEnsuresScope,
        accepted_kinds: &[HirIdKind::Item],
        source_ordered: false,
        tag: 0x0d,
    },
    RoleCase {
        role: SyntheticRole::ForIterator,
        accepted_kinds: &[HirIdKind::Stmt],
        source_ordered: false,
        tag: 0x0e,
    },
    RoleCase {
        role: SyntheticRole::ForNextValue,
        accepted_kinds: &[HirIdKind::Stmt],
        source_ordered: false,
        tag: 0x0f,
    },
    RoleCase {
        role: SyntheticRole::IfLetScrutinee,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Stmt],
        source_ordered: false,
        tag: 0x10,
    },
    RoleCase {
        role: SyntheticRole::WhileLetScrutinee,
        accepted_kinds: &[HirIdKind::Stmt],
        source_ordered: false,
        tag: 0x11,
    },
    RoleCase {
        role: SyntheticRole::MatchScrutinee,
        accepted_kinds: &[HirIdKind::Expr, HirIdKind::Stmt],
        source_ordered: false,
        tag: 0x12,
    },
    RoleCase {
        role: SyntheticRole::PatternRest,
        accepted_kinds: &[HirIdKind::Pattern],
        source_ordered: false,
        tag: 0x13,
    },
    RoleCase {
        role: SyntheticRole::PostfixIndexCandidateExpression,
        accepted_kinds: &[HirIdKind::Expr],
        source_ordered: true,
        tag: 0x14,
    },
    RoleCase {
        role: SyntheticRole::DialogueContentCandidateExpression,
        accepted_kinds: &[HirIdKind::Expr],
        source_ordered: true,
        tag: 0x15,
    },
];

#[test]
fn synthetic_roles_admit_the_complete_typed_owner_and_ordinal_matrix() {
    let module = module_id(23, 29);
    for case in ROLE_CASES {
        assert_eq!(case.role.fingerprint_tag(), case.tag);
        assert!(case.role.accepts_ordinal(0));

        let accepted_ordinals: &[u32] = if case.source_ordered {
            &[0, 1_023]
        } else {
            &[0]
        };
        let rejected_ordinals: &[u32] = if case.source_ordered {
            &[1_024, u32::MAX]
        } else {
            &[1, u32::MAX]
        };

        for kind in ALL_KINDS {
            let accepted_kind = case.accepted_kinds.contains(&kind);
            assert_eq!(case.role.accepts_owner_kind(kind), accepted_kind);
            let typed_owner = owner(module, kind as u32 + 1, kind);

            for ordinal in accepted_ordinals {
                assert_eq!(case.role.accepts_owner(kind, *ordinal), accepted_kind);
                if accepted_kind {
                    let key = SyntheticKey::try_new(typed_owner, case.role, *ordinal).unwrap();
                    assert_eq!(key.owner(), typed_owner);
                    assert_eq!(key.role(), case.role);
                    assert_eq!(key.ordinal(), *ordinal);
                } else {
                    assert_eq!(
                        SyntheticKey::try_new(typed_owner, case.role, *ordinal),
                        Err(SyntheticKeyError::WrongOwnerKind {
                            role: case.role,
                            actual: kind,
                        })
                    );
                }
            }

            for ordinal in rejected_ordinals {
                assert!(!case.role.accepts_ordinal(*ordinal));
                assert!(!case.role.accepts_owner(kind, *ordinal));
                let expected = if accepted_kind {
                    SyntheticKeyError::InvalidOrdinal {
                        role: case.role,
                        ordinal: *ordinal,
                    }
                } else {
                    SyntheticKeyError::WrongOwnerKind {
                        role: case.role,
                        actual: kind,
                    }
                };
                assert_eq!(
                    SyntheticKey::try_new(typed_owner, case.role, *ordinal),
                    Err(expected)
                );
            }
        }
    }
}

#[test]
fn synthetic_key_errors_prioritize_owner_kind_and_round_trip_valid_keys() {
    let module = module_id(31, 37);
    let expression = owner(module, 41, HirIdKind::Expr);
    let type_node = owner(module, 43, HirIdKind::Type);

    let wrong_owner =
        SyntheticKey::try_new(expression, SyntheticRole::ElidedRegion, 1).unwrap_err();
    assert_eq!(
        wrong_owner,
        SyntheticKeyError::WrongOwnerKind {
            role: SyntheticRole::ElidedRegion,
            actual: HirIdKind::Expr,
        }
    );
    assert_eq!(
        wrong_owner.to_string(),
        "synthetic role ElidedRegion does not accept owner kind Expr"
    );

    let wrong_ordinal =
        SyntheticKey::try_new(type_node, SyntheticRole::ElidedRegion, 1).unwrap_err();
    assert_eq!(
        wrong_ordinal,
        SyntheticKeyError::InvalidOrdinal {
            role: SyntheticRole::ElidedRegion,
            ordinal: 1,
        }
    );
    assert_eq!(
        wrong_ordinal.to_string(),
        "synthetic role ElidedRegion does not accept ordinal 1"
    );

    let key = SyntheticKey::try_new(type_node, SyntheticRole::ElidedRegion, 0).unwrap();
    assert_eq!(key.owner(), type_node);
    assert_eq!(key.role(), SyntheticRole::ElidedRegion);
    assert_eq!(key.ordinal(), 0);
}

#[test]
fn synthetic_keys_have_complete_structural_identity() {
    fn assert_structural_traits<
        T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
    >() {
    }

    assert_structural_traits::<SyntheticKey>();
    assert_structural_traits::<SyntheticKeyError>();
    assert_structural_traits::<SyntheticKeyFingerprintInput>();

    let baseline = SyntheticKey::try_new(
        owner(module_id(1, 2), 3, HirIdKind::Expr),
        SyntheticRole::RecoveryOperand,
        0,
    )
    .unwrap();
    let keys = [
        baseline,
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Stmt),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(2, 2), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 4), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 5, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Expr),
            SyntheticRole::DesugaredTemporary,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            1,
        )
        .unwrap(),
    ];

    let mut hash_keys = HashSet::from(keys);
    assert!(!hash_keys.insert(baseline));
    assert_eq!(hash_keys.len(), keys.len());
    assert_eq!(BTreeSet::from(keys).len(), keys.len());
}

#[test]
fn synthetic_key_fingerprint_matches_fixed_vectors_and_exact_tags() {
    let vector_a = SyntheticKey::try_new(
        owner(module_id(1, 2), 3, HirIdKind::Type),
        SyntheticRole::ElidedRegion,
        0,
    )
    .unwrap()
    .fingerprint_input();
    assert_eq!(vector_a.as_bytes().len(), 51);
    assert_eq!(
        fingerprint_hex(vector_a),
        "617263776566742d6869722d73796e7468657469632d6b65792d76310006010000000000000002000000030000000400000000"
    );

    let vector_b = SyntheticKey::try_new(
        owner(
            module_id(0x0102_0304_0506_0708, 0x0a0b_0c0d),
            0x1112_1314,
            HirIdKind::Expr,
        ),
        SyntheticRole::DialogueContentCandidateExpression,
        7,
    )
    .unwrap()
    .fingerprint_input();
    assert_eq!(
        fingerprint_hex(vector_b),
        "617263776566742d6869722d73796e7468657469632d6b65792d7631000408070605040302010d0c0b0a141312111507000000"
    );

    let module = module_id(47, 53);
    let owner_tags = [
        owner(module, 1, HirIdKind::Item),
        owner(module, 2, HirIdKind::Scope),
        owner(module, 3, HirIdKind::Local),
        owner(module, 4, HirIdKind::Expr),
        owner(module, 5, HirIdKind::Stmt),
        owner(module, 6, HirIdKind::Type),
        owner(module, 7, HirIdKind::Pattern),
        owner(module, 8, HirIdKind::Capture),
    ]
    .map(SyntheticOwner::fingerprint_tag);
    assert_eq!(owner_tags, [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn synthetic_key_fingerprint_separates_every_field_and_database_session() {
    let baseline = SyntheticKey::try_new(
        owner(module_id(1, 2), 3, HirIdKind::Expr),
        SyntheticRole::RecoveryOperand,
        0,
    )
    .unwrap();
    let mutations = [
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Stmt),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(2, 2), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 4), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 5, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Expr),
            SyntheticRole::DesugaredTemporary,
            0,
        )
        .unwrap(),
        SyntheticKey::try_new(
            owner(module_id(1, 2), 3, HirIdKind::Expr),
            SyntheticRole::RecoveryOperand,
            1,
        )
        .unwrap(),
    ];

    let baseline_input = baseline.fingerprint_input();
    assert_eq!(baseline_input, baseline.fingerprint_input());
    let distinct = mutations.map(SyntheticKey::fingerprint_input);
    assert!(distinct.iter().all(|input| input != &baseline_input));
    assert_eq!(BTreeSet::from(distinct).len(), distinct.len());
    assert_ne!(
        &baseline_input.as_bytes()[30..38],
        &distinct[1].as_bytes()[30..38]
    );
}
