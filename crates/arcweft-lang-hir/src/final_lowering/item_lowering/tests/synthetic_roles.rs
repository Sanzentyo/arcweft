use std::collections::{BTreeMap, BTreeSet};

use super::*;

use crate::identity::{CaptureId, HirIdKind, ItemId, LocalId, PatternId, RawHirId, StmtId, TypeId};
use crate::item::{HirFunctionBody, HirProofBody};
use crate::source_index::{
    HirSourcePresence, HirSourceQuery, HirTypeRegionSourcePart, HirTypeSourceRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SyntheticObservation {
    child: RawHirId,
    insertion_offset: usize,
    poisoned: bool,
}

type SyntheticObservationKey = (SyntheticKey, HirIdKind);

const SYNTHETIC_SLOT_ROLES: [SyntheticRole; 8] = [
    SyntheticRole::ImplicitUnitTail,
    SyntheticRole::PredicateBoolReturn,
    SyntheticRole::ProofUnitReturn,
    SyntheticRole::RecoveryOperand,
    SyntheticRole::PostconditionResult,
    SyntheticRole::ClosureCapture,
    SyntheticRole::ContractRequiresScope,
    SyntheticRole::ContractEnsuresScope,
];

fn insertion_offset(site: &HirSourceSite) -> usize {
    let HirSourceSite::Insertion(insertion) = site else {
        panic!("synthetic fixture must retain its prescribed zero-width insertion")
    };
    insertion.offset()
}

fn record_synthetic<I: HirTypedId>(
    module: &HirModule,
    observations: &mut BTreeMap<SyntheticObservationKey, SyntheticObservation>,
) {
    for child in module.slots().prepared_live_ids::<I>() {
        let metadata = module.slots().resolve(child).unwrap();
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            continue;
        };
        let previous = observations.insert(
            (*key, I::KIND),
            SyntheticObservation {
                child: child.raw(),
                insertion_offset: insertion_offset(metadata.source_site()),
                poisoned: metadata.is_poisoned(),
            },
        );
        assert!(
            previous.is_none(),
            "one exact (SyntheticKey, HirIdKind) pair must own one child"
        );
    }
}

fn synthetic_observations(
    module: &HirModule,
) -> BTreeMap<SyntheticObservationKey, SyntheticObservation> {
    let mut observations = BTreeMap::new();
    record_synthetic::<ItemId>(module, &mut observations);
    record_synthetic::<ScopeId>(module, &mut observations);
    record_synthetic::<LocalId>(module, &mut observations);
    record_synthetic::<ExprId>(module, &mut observations);
    record_synthetic::<StmtId>(module, &mut observations);
    record_synthetic::<TypeId>(module, &mut observations);
    record_synthetic::<PatternId>(module, &mut observations);
    record_synthetic::<CaptureId>(module, &mut observations);
    observations
}

fn observation(
    observations: &BTreeMap<SyntheticObservationKey, SyntheticObservation>,
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
    child_kind: HirIdKind,
) -> SyntheticObservation {
    observations
        .get(&(
            SyntheticKey::try_new(owner, role, ordinal).unwrap(),
            child_kind,
        ))
        .copied()
        .unwrap_or_else(|| {
            panic!("missing synthetic observation for {owner:?}/{role:?}/{ordinal}/{child_kind:?}")
        })
}

fn assert_elided_region_anchors(
    module: &HirModule,
    parsed: &ParsedSource,
    expected_offsets: &BTreeSet<usize>,
) -> BTreeSet<SyntheticKey> {
    let keys = module
        .slots()
        .key_only_synthetic_keys()
        .filter(|key| key.role() == SyntheticRole::ElidedRegion)
        .collect::<BTreeSet<_>>();
    let offsets = keys
        .iter()
        .map(|key| {
            assert_eq!(key.ordinal(), 0);
            let SyntheticOwner::Type(owner) = key.owner() else {
                panic!("ElidedRegion must remain Type-owned")
            };
            let source = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Type {
                        owner,
                        role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion),
                    },
                )
                .expect("elided region source insertion");
            let HirSourcePresence::Present(site) = source.presence() else {
                panic!("elided region must retain an exact insertion")
            };
            insertion_offset(site)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(&offsets, expected_offsets);
    keys
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the synthetic-role test exhausts produced roles, source anchors, and collision stability"
)]
fn produced_synthetic_roles_are_stable_and_collision_free() {
    const SOURCE: &str = concat!(
        "fn roles(left: &Int, right: &Int) -> ()\n",
        "requires left == right\n",
        "ensures result == ()\n",
        "{\n",
        "    let first = left;\n",
        "    let second = right;\n",
        "    let piped = left |> right;\n",
        "    let closure = || second + first + second;\n",
        "    let recovered = -;\n",
        "}\n",
        "predicate valid(value: &Int)\n",
        "requires value == value\n",
        "ensures result\n",
        "= true\n",
        "proof checked(value: &Int)\n",
        "requires value == value\n",
        "ensures result == ()\n",
        "{}\n",
    );
    const UNRELATED_SIBLING: &str = "fn unrelated() {}\n";

    let name = SourceName::path("proof/synthetic-role-stability.arcw");
    let document_id = "arcweft-test://proof/synthetic-role-stability";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, SOURCE),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let first_observations = synthetic_observations(&first);

    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(SOURCE.len(), SOURCE.len()))
                    .unwrap(),
                UNRELATED_SIBLING,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second = lower(&mut database, &revised, &key);
    let second_observations = synthetic_observations(&second);

    for role in SYNTHETIC_SLOT_ROLES {
        assert!(
            first_observations.keys().any(|(key, _)| key.role() == role),
            "fixture must exercise {role:?} through a typed slot"
        );
    }
    for (key, first_observation) in &first_observations {
        if !SYNTHETIC_SLOT_ROLES.contains(&key.0.role()) {
            continue;
        }
        let second_observation = second_observations
            .get(key)
            .expect("unrelated sibling edit must preserve the old synthetic key");
        assert_eq!(second_observation.child, first_observation.child);
        assert_eq!(
            second_observation.insertion_offset,
            first_observation.insertion_offset
        );
        assert_eq!(second_observation.poisoned, first_observation.poisoned);
    }

    let first_children = first_observations
        .values()
        .map(|observation| observation.child)
        .collect::<BTreeSet<_>>();
    assert_eq!(first_children.len(), first_observations.len());

    let function_owner = first.source_ordered_items()[0];
    let predicate_owner = first.source_ordered_items()[1];
    let proof_owner = first.source_ordered_items()[2];
    let requires = observation(
        &first_observations,
        SyntheticOwner::Item(function_owner),
        SyntheticRole::ContractRequiresScope,
        0,
        HirIdKind::Scope,
    );
    let ensures = observation(
        &first_observations,
        SyntheticOwner::Item(function_owner),
        SyntheticRole::ContractEnsuresScope,
        0,
        HirIdKind::Scope,
    );
    assert_eq!(requires.insertion_offset, SOURCE.find("requires").unwrap());
    assert_eq!(ensures.insertion_offset, SOURCE.find("ensures").unwrap());

    let predicate_return = observation(
        &first_observations,
        SyntheticOwner::Item(predicate_owner),
        SyntheticRole::PredicateBoolReturn,
        0,
        HirIdKind::Type,
    );
    assert_eq!(
        predicate_return.insertion_offset,
        SOURCE.find("predicate valid(value: &Int)").unwrap() + "predicate valid(value: &Int)".len()
    );
    let proof_return = observation(
        &first_observations,
        SyntheticOwner::Item(proof_owner),
        SyntheticRole::ProofUnitReturn,
        0,
        HirIdKind::Type,
    );
    assert_eq!(
        proof_return.insertion_offset,
        SOURCE.find("proof checked(value: &Int)").unwrap() + "proof checked(value: &Int)".len()
    );

    let ensures_scope = ScopeId::from_raw(ensures.child);
    let postcondition_result = observation(
        &first_observations,
        SyntheticOwner::Scope(ensures_scope),
        SyntheticRole::PostconditionResult,
        0,
        HirIdKind::Local,
    );
    assert_eq!(
        postcondition_result.insertion_offset,
        SOURCE.find("result == ()").unwrap()
    );

    let function = resolve_item(&first, 0);
    let HirItemKind::Function(function) = function.kind() else {
        panic!("first fixture item must remain a Function")
    };
    let HirFunctionBody::Block { scope, tail, .. } = function.body() else {
        panic!("fixture Function must retain its block body")
    };
    let implicit_tail = observation(
        &first_observations,
        SyntheticOwner::Scope(*scope),
        SyntheticRole::ImplicitUnitTail,
        0,
        HirIdKind::Expr,
    );
    assert_eq!(implicit_tail.child, tail.raw());
    assert_eq!(
        implicit_tail.insertion_offset,
        SOURCE.find("\n}\npredicate").unwrap() + 1
    );

    let proof = resolve_item(&first, 2);
    let HirItemKind::Proof(proof) = proof.kind() else {
        panic!("third fixture item must remain a Proof")
    };
    let HirProofBody::Block { tail, .. } = proof.body() else {
        panic!("fixture Proof must retain its block body")
    };
    assert!(matches!(
        first.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner().kind() == HirIdKind::Scope
                && key.role() == SyntheticRole::ImplicitUnitTail
                && key.ordinal() == 0
    ));

    let capture_rows = first_observations
        .iter()
        .filter(|((key, kind), _)| {
            key.role() == SyntheticRole::ClosureCapture && *kind == HirIdKind::Capture
        })
        .collect::<Vec<_>>();
    assert_eq!(capture_rows.len(), 2);
    assert_eq!(capture_rows[0].0.0.ordinal(), 0);
    assert_eq!(capture_rows[1].0.0.ordinal(), 1);
    assert_eq!(capture_rows[0].0.0.owner(), capture_rows[1].0.0.owner());
    let closure_body = SOURCE.find("second + first + second").unwrap();
    assert_eq!(capture_rows[0].1.insertion_offset, closure_body);
    assert_eq!(
        capture_rows[1].1.insertion_offset,
        closure_body + "second + ".len()
    );

    let recovery_rows = first_observations
        .iter()
        .filter(|((key, _), _)| key.role() == SyntheticRole::RecoveryOperand)
        .collect::<Vec<_>>();
    assert_eq!(recovery_rows.len(), 1);
    assert_eq!(recovery_rows[0].0.0.ordinal(), 0);
    assert_eq!(
        recovery_rows[0].1.insertion_offset,
        SOURCE.find("-;").unwrap() + 1
    );
    assert!(recovery_rows[0].1.poisoned);

    let expected_elision_offsets = SOURCE
        .match_indices('&')
        .map(|(offset, _)| offset + 1)
        .collect::<BTreeSet<_>>();
    let first_elided = assert_elided_region_anchors(&first, &initial, &expected_elision_offsets);
    let second_elided = assert_elided_region_anchors(&second, &revised, &expected_elision_offsets);
    assert!(first_elided.is_subset(&second_elided));
}
