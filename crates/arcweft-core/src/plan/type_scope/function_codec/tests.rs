use super::super::source_codec::RuntimeTypeScopeCodecLimits;
use super::*;
use crate::effect_row::{DecisionControl, DecisionWork, EffectSet};
use crate::plan::RuntimeTypeScope;

struct Algebra;
impl DecisionControl for Algebra {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn limits() -> RuntimeFunctionTypeSourceLimits {
    RuntimeFunctionTypeSourceLimits {
        encoded_bytes: 65536,
        decoded_bytes: 65536,
        decision_nodes: 128,
        overrides: 128,
    }
}
fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}
fn encode(contract: &RuntimeFunctionTypeContract, incoming: &RuntimeTypeScope) -> Vec<u8> {
    let prepared = contract
        .prepare_source_encoding(incoming, limits(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn fixture() -> (RuntimeTypeScope, RuntimeFunctionTypeContract) {
    let incoming = RuntimeTypeScope::root()
        .enter(RuntimeTypeBinder::new(1, 1, 1))
        .unwrap();
    let binder = RuntimeTypeBinder::new(2, 3, 2);
    let child = incoming.enter(binder).unwrap();
    let first = EffectFormula::variable(child.bound_effect(0, 0).unwrap(), &mut Algebra).unwrap();
    let second = EffectFormula::variable(child.bound_effect(0, 1).unwrap(), &mut Algebra).unwrap();
    let outer = EffectFormula::variable(child.bound_effect(1, 0).unwrap(), &mut Algebra).unwrap();
    let literal = EffectFormula::literal(
        EffectSet::from_labels(["fs.read(save)", "state.write(flow)"]).unwrap(),
        None,
    );
    let predicate = first.subset(&second, &mut Algebra).unwrap();
    let invocation = first
        .union(&outer, &mut Algebra)
        .unwrap()
        .union(&literal, &mut Algebra)
        .unwrap();
    (
        incoming,
        RuntimeFunctionTypeContract::new(binder, predicate, invocation),
    )
}

#[test]
fn original_nested_effect_graph_and_function_binder_round_trip() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    let mut bank = meter();
    let prepared =
        RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, limits(), &mut bank)
            .unwrap();
    let retained = prepared.materialize(&mut bank).unwrap();
    assert_eq!(retained, contract);
    assert_eq!(
        retained.child_scope(&incoming),
        contract.child_scope(&incoming)
    );
    assert_eq!(encode(&retained, &incoming), bytes);
    assert!(
        retained
            .invocation()
            .variables()
            .any(|reference| reference.depth() == 1)
    );
}

#[test]
fn canonical_version_one_wire_has_binder_predicate_and_invocation_source() {
    let bytes = encode(
        &RuntimeFunctionTypeContract::default(),
        &RuntimeTypeScope::root(),
    );
    assert_eq!(
        bytes,
        [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
}

#[test]
fn original_scope_and_function_frames_preflight_together_before_scope_retention() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    let scope_limits = RuntimeTypeScopeCodecLimits {
        encoded_bytes: 1024,
        decoded_bytes: 1024,
    };
    let prepared = incoming
        .prepare_source_encoding(scope_limits, &mut meter())
        .unwrap();
    let mut scope_bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut scope_bytes).unwrap();
    let mut bank = meter();
    let original_scope =
        RuntimeTypeScope::preflight_source(&scope_bytes, scope_limits, &mut bank).unwrap();
    let scope_ref = original_scope.as_scope_ref();
    assert_eq!(scope_ref.bound_type(0, 0), incoming.bound_type(0, 0));
    assert_eq!(scope_ref.bound_const(0, 0), incoming.bound_const(0, 0));
    assert_eq!(scope_ref.bound_effect(0, 0), incoming.bound_effect(0, 0));
    let source =
        RuntimeFunctionTypeContract::preflight_source(&bytes, scope_ref, limits(), &mut bank)
            .unwrap();
    // Both complete source frames have now been checked, while the incoming
    // binders still borrow their original immutable bytes rather than a Vec.
    let retained = source.materialize(&mut bank).unwrap();
    let retained_scope = original_scope.materialize();
    assert_eq!(retained, contract);
    assert_eq!(retained_scope, incoming);
    assert_eq!(
        retained.child_scope(&retained_scope),
        contract.child_scope(&incoming)
    );
    let end = scope_bytes.len();
    scope_bytes[end - 4..].fill(0);
    let mut bank = meter();
    let wrong_scope =
        RuntimeTypeScope::preflight_source(&scope_bytes, scope_limits, &mut bank).unwrap();
    assert!(matches!(
        RuntimeFunctionTypeContract::preflight_source(
            &bytes,
            wrong_scope.as_scope_ref(),
            limits(),
            &mut bank
        ),
        Err(RuntimeFunctionTypeSourceError::Scope(
            RuntimeTypeScopeError::EffectSlot { arity: 0, .. }
        ))
    ));
}

#[test]
fn all_truncations_unknown_tags_counts_and_trailing_data_fail_before_retention() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    for cut in 0..bytes.len() {
        assert!(
            RuntimeFunctionTypeContract::preflight_source(
                &bytes[..cut],
                &incoming,
                limits(),
                &mut meter()
            )
            .is_err(),
            "cut {cut}"
        );
    }
    for (offset, replacement) in [(0, 2), (9, 255)] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = replacement;
        assert!(
            RuntimeFunctionTypeContract::preflight_source(
                &corrupt,
                &incoming,
                limits(),
                &mut meter()
            )
            .is_err()
        );
    }
    let root = RuntimeTypeScope::root();
    let mut tag = encode(&RuntimeFunctionTypeContract::default(), &root);
    tag[13] = 255;
    assert_eq!(
        RuntimeFunctionTypeContract::preflight_source(&tag, &root, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeFunctionTypeSourceError::Tag { tag: 255 }
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        RuntimeFunctionTypeContract::preflight_source(&trailing, &incoming, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeFunctionTypeSourceError::Trailing
    );
}

#[test]
fn original_incoming_scope_and_new_binder_admit_distinct_reference_depths() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    assert!(
        RuntimeFunctionTypeContract::preflight_source(
            &bytes,
            &RuntimeTypeScope::root(),
            limits(),
            &mut meter()
        )
        .is_err()
    );
    assert!(
        contract
            .prepare_source_encoding(&RuntimeTypeScope::root(), limits(), &mut meter())
            .is_err()
    );
    for (offset, value) in [(13, u32::MAX), (17, u32::MAX)] {
        let mut corrupt = bytes.clone();
        corrupt[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            RuntimeFunctionTypeContract::preflight_source(
                &corrupt,
                &incoming,
                limits(),
                &mut meter()
            ),
            Err(RuntimeFunctionTypeSourceError::Scope(_))
        ));
    }
    let outer =
        EffectFormula::variable(incoming.bound_effect(0, 0).unwrap(), &mut Algebra).unwrap();
    let empty_binder = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::EMPTY,
        EffectPredicate::unconstrained(),
        outer,
    );
    let bytes = encode(&empty_binder, &incoming);
    let mut bank = meter();
    let retained =
        RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, limits(), &mut bank)
            .unwrap()
            .materialize(&mut bank)
            .unwrap();
    assert_eq!(retained.child_scope(&incoming).unwrap(), incoming);
}

#[test]
fn function_binder_depth_limit_is_checked_without_copying_the_incoming_scope() {
    let incoming = RuntimeTypeScope::try_from_binders(vec![
        RuntimeTypeBinder::new(1, 0, 0);
        MAX_RUNTIME_PLAN_TYPE_DEPTH
    ])
    .unwrap();
    let empty = RuntimeFunctionTypeContract::default();
    let bytes = encode(&empty, &incoming);
    let mut bank = meter();
    RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, limits(), &mut bank)
        .unwrap()
        .materialize(&mut bank)
        .unwrap();
    let binder = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::new(1, 0, 0),
        EffectPredicate::unconstrained(),
        EffectFormula::literal(EffectSet::new(), None),
    );
    assert_eq!(
        binder
            .prepare_source_encoding(&incoming, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeFunctionTypeSourceError::Scope(RuntimeTypeScopeError::DepthLimit)
    );
    let bytes = encode(&binder, &RuntimeTypeScope::root());
    assert_eq!(
        RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeFunctionTypeSourceError::Scope(RuntimeTypeScopeError::DepthLimit)
    );
}

#[test]
fn labels_are_validated_as_original_canonical_source_in_the_complete_preflight() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    let label = b"state.write(flow)";
    let offset = bytes
        .windows(label.len())
        .position(|bytes| bytes == label)
        .unwrap();
    for replacement in [255, b'S', b' '] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = replacement;
        assert_eq!(
            RuntimeFunctionTypeContract::preflight_source(
                &corrupt,
                &incoming,
                limits(),
                &mut meter()
            )
            .err()
            .unwrap(),
            RuntimeFunctionTypeSourceError::EffectLabel
        );
    }
}

#[test]
fn both_membership_graphs_share_exact_node_override_and_storage_caps() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    let mut count = Reader::new(
        &bytes,
        (&incoming).into(),
        Counts::new(limits(), 2).unwrap(),
    );
    count.contract(false).unwrap();
    let exact = RuntimeFunctionTypeSourceLimits {
        encoded_bytes: bytes.len(),
        decoded_bytes: count.counts.storage,
        decision_nodes: u32::try_from(count.counts.nodes).unwrap(),
        overrides: u32::try_from(count.counts.overrides).unwrap(),
    };
    assert!(exact.decision_nodes > 1 && exact.overrides > 1);
    let prepared = contract
        .prepare_source_encoding(&incoming, exact, &mut meter())
        .unwrap();
    assert_eq!(prepared.storage_bytes(), exact.decoded_bytes);
    assert_eq!(prepared.encoded_bytes(), bytes.len());
    let mut bank = meter();
    let prepared =
        RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, exact, &mut bank).unwrap();
    assert_eq!(prepared.storage_bytes(), exact.decoded_bytes);
    prepared.materialize(&mut bank).unwrap();
    for short in [
        RuntimeFunctionTypeSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            decision_nodes: exact.decision_nodes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            overrides: exact.overrides - 1,
            ..exact
        },
    ] {
        let encode_error = contract
            .prepare_source_encoding(&incoming, short, &mut meter())
            .err()
            .unwrap();
        let decode_error =
            RuntimeFunctionTypeContract::preflight_source(&bytes, &incoming, short, &mut meter())
                .err()
                .unwrap();
        assert!(matches!(
            encode_error,
            RuntimeFunctionTypeSourceError::Limit { .. }
        ));
        assert!(matches!(
            decode_error,
            RuntimeFunctionTypeSourceError::Limit { .. }
        ));
    }
}

#[test]
fn noncanonical_graphs_and_infinite_or_impossible_terms_fail_owning_admission() {
    let root = RuntimeTypeScope::root();
    let canonical = encode(&RuntimeFunctionTypeContract::default(), &root);
    for (offset, tag, expected) in [
        (
            13,
            0,
            RuntimeFunctionTypeSourceError::Scope(RuntimeTypeScopeError::ImpossiblePredicate),
        ),
        (
            22,
            1,
            RuntimeFunctionTypeSourceError::Declaration(EffectDeclarationError::InfiniteRow),
        ),
    ] {
        let mut corrupt = canonical.clone();
        corrupt[offset] = tag;
        let mut bank = meter();
        let prepared =
            RuntimeFunctionTypeContract::preflight_source(&corrupt, &root, limits(), &mut bank)
                .unwrap();
        assert_eq!(prepared.materialize(&mut bank), Err(expected));
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
    // An inert unreachable node passes bounded wire admission, then fails the
    // same canonical graph owner used by every other source producer.
    let unreachable = [
        1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ];
    let mut bank = meter();
    let prepared =
        RuntimeFunctionTypeContract::preflight_source(&unreachable, &root, limits(), &mut bank)
            .unwrap();
    assert_eq!(
        prepared.materialize(&mut bank),
        Err(RuntimeFunctionTypeSourceError::Declaration(
            EffectDeclarationError::NonCanonicalPostorder
        ))
    );
}

#[test]
fn parent_append_requires_reserved_capacity_and_preserves_its_prefix() {
    let (incoming, contract) = fixture();
    let bytes = encode(&contract, &incoming);
    let mut output = Vec::with_capacity(bytes.len() + 3);
    output.extend_from_slice(&[5, 6, 7]);
    let pointer = output.as_ptr();
    contract
        .prepare_source_encoding(&incoming, limits(), &mut meter())
        .unwrap()
        .append_to(&mut output)
        .unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(&output[..3], [5, 6, 7]);
    assert_eq!(&output[3..], bytes);
    let mut absent = Vec::new();
    assert!(matches!(
        contract
            .prepare_source_encoding(&incoming, limits(), &mut meter())
            .unwrap()
            .append_to(&mut absent),
        Err(RuntimeFunctionTypeSourceError::OutputCapacity { .. })
    ));
    assert_eq!(absent.capacity(), 0);
}

#[test]
fn source_passes_keep_the_original_work_bank_and_first_error() {
    let root = RuntimeTypeScope::root();
    let contract = RuntimeFunctionTypeContract::default();
    let bytes = encode(&contract, &root);
    let work = 3 * u64::try_from(bytes.len()).unwrap();
    let mut bank = TaskSemanticMeter::new(work + 1, u64::MAX);
    bank.charge_work(1).unwrap();
    RuntimeFunctionTypeContract::preflight_source(&bytes, &root, limits(), &mut bank)
        .unwrap()
        .materialize(&mut bank)
        .unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    contract
        .prepare_source_encoding(&root, limits(), &mut bank)
        .unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    for decoding in [true, false] {
        let mut short = TaskSemanticMeter::new(work - 1, u64::MAX);
        let error = if decoding {
            RuntimeFunctionTypeContract::preflight_source(&bytes, &root, limits(), &mut short)
                .err()
                .unwrap()
        } else {
            contract
                .prepare_source_encoding(&root, limits(), &mut short)
                .err()
                .unwrap()
        };
        assert_eq!(
            error,
            RuntimeFunctionTypeSourceError::Resource(TaskSemanticEncodingError::SemanticWork)
        );
        assert_eq!(
            RuntimeFunctionTypeContract::preflight_source(&[], &root, limits(), &mut short)
                .err()
                .unwrap(),
            error
        );
    }
}

fn encode_formula(
    formula: &EffectFormula<RuntimeBoundEffectReference>,
    scope: &RuntimeTypeScope,
) -> Vec<u8> {
    let prepared = formula
        .prepare_source_encoding(scope, limits(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut output).unwrap();
    output
}

#[test]
fn effect_argument_source_preserves_the_actual_target_scope_and_original_graph() {
    let (incoming, contract) = fixture();
    let target = contract.child_scope(&incoming).unwrap();
    let formula = contract.invocation();
    let bytes = encode_formula(formula, &target);
    let mut bank = meter();
    let encoding = formula
        .prepare_source_encoding(&target, limits(), &mut meter())
        .unwrap();
    let prepared = EffectFormula::preflight_source(&bytes, &target, limits(), &mut bank).unwrap();
    assert_eq!(prepared.storage_bytes(), encoding.storage_bytes());
    assert_eq!(prepared.elements(), encoding.elements());
    let retained = prepared.materialize(&mut bank).unwrap();
    assert_eq!(&retained, formula);
    assert_eq!(encode_formula(&retained, &target), bytes);
    let encoding = contract
        .prepare_argument_source_encoding(formula, &incoming, limits(), &mut meter())
        .unwrap();
    let mut borrowed_context = Vec::with_capacity(encoding.encoded_bytes());
    encoding.append_to(&mut borrowed_context).unwrap();
    assert_eq!(borrowed_context, bytes);
    let mut bank = meter();
    let prepared = contract
        .preflight_argument_source_in_frame(&bytes, &incoming, limits())
        .unwrap();
    assert_eq!(prepared.materialize(&mut bank).unwrap(), *formula);
    assert!(retained.variables().any(|reference| reference.depth() == 1));
    assert!(
        EffectFormula::preflight_source(&bytes, &RuntimeTypeScope::root(), limits(), &mut meter())
            .is_err()
    );
    assert!(
        formula
            .prepare_source_encoding(&RuntimeTypeScope::root(), limits(), &mut meter())
            .is_err()
    );
}

#[test]
fn effect_argument_source_uses_one_real_membership_graph_and_exact_caps() {
    let (incoming, contract) = fixture();
    let scope = contract.child_scope(&incoming).unwrap();
    let formula = contract.invocation();
    let bytes = encode_formula(formula, &scope);
    let prepared = formula
        .prepare_source_encoding(&scope, limits(), &mut meter())
        .unwrap();
    let declaration = Membership::try_from(formula).unwrap();
    let nodes = declaration.default.nodes.len()
        + declaration
            .overrides
            .iter()
            .map(|(_, graph)| graph.nodes.len())
            .sum::<usize>();
    let exact = RuntimeFunctionTypeSourceLimits {
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        decision_nodes: nodes as u32,
        overrides: declaration.overrides.len() as u32,
    };
    assert_eq!(prepared.elements(), nodes + declaration.overrides.len());
    formula
        .prepare_source_encoding(&scope, exact, &mut meter())
        .unwrap();
    let mut bank = meter();
    EffectFormula::preflight_source(&bytes, &scope, exact, &mut bank)
        .unwrap()
        .materialize(&mut bank)
        .unwrap();
    for short in [
        RuntimeFunctionTypeSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            decision_nodes: exact.decision_nodes - 1,
            ..exact
        },
        RuntimeFunctionTypeSourceLimits {
            overrides: exact.overrides - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            formula.prepare_source_encoding(&scope, short, &mut meter()),
            Err(RuntimeFunctionTypeSourceError::Limit { .. })
        ));
        assert!(matches!(
            EffectFormula::preflight_source(&bytes, &scope, short, &mut meter()),
            Err(RuntimeFunctionTypeSourceError::Limit { .. })
        ));
    }
    let root = RuntimeTypeScope::root();
    let empty = EffectFormula::empty();
    let prepared = empty
        .prepare_source_encoding(&root, limits(), &mut meter())
        .unwrap();
    assert_eq!(
        prepared.storage_bytes(),
        Decision::source_admission_storage(0).unwrap()
    );
    assert_eq!(
        encode_formula(&empty, &root),
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn effect_argument_source_borrows_the_original_scope_frame_before_retention() {
    let (incoming, contract) = fixture();
    let scope = contract.child_scope(&incoming).unwrap();
    let bytes = encode_formula(contract.invocation(), &scope);
    let scope_limits = RuntimeTypeScopeCodecLimits {
        encoded_bytes: 1024,
        decoded_bytes: 1024,
    };
    let encoding = scope
        .prepare_source_encoding(scope_limits, &mut meter())
        .unwrap();
    let mut scope_bytes = Vec::with_capacity(encoding.encoded_bytes());
    encoding.append_to(&mut scope_bytes).unwrap();
    let mut bank = meter();
    let original =
        RuntimeTypeScope::preflight_source(&scope_bytes, scope_limits, &mut bank).unwrap();
    let prepared =
        EffectFormula::preflight_source(&bytes, original.as_scope_ref(), limits(), &mut bank)
            .unwrap();
    assert_eq!(
        prepared.materialize(&mut bank).unwrap(),
        *contract.invocation()
    );
    assert_eq!(original.materialize(), scope);
}

#[test]
fn effect_argument_source_rejects_malformed_frames_and_owning_graph_errors() {
    let (incoming, contract) = fixture();
    let scope = contract.child_scope(&incoming).unwrap();
    let bytes = encode_formula(contract.invocation(), &scope);
    for cut in 0..bytes.len() {
        assert!(
            EffectFormula::preflight_source(&bytes[..cut], &scope, limits(), &mut meter()).is_err()
        );
    }
    for (offset, replacement) in [(0, 2), (1, 255)] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = replacement;
        assert!(EffectFormula::preflight_source(&corrupt, &scope, limits(), &mut meter()).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        EffectFormula::preflight_source(&trailing, &scope, limits(), &mut meter()),
        Err(RuntimeFunctionTypeSourceError::Trailing)
    ));
    let label = b"state.write(flow)";
    let offset = bytes
        .windows(label.len())
        .position(|part| part == label)
        .unwrap();
    let mut corrupt = bytes;
    corrupt[offset] = 255;
    assert!(matches!(
        EffectFormula::preflight_source(&corrupt, &scope, limits(), &mut meter()),
        Err(RuntimeFunctionTypeSourceError::EffectLabel)
    ));
    let root = RuntimeTypeScope::root();
    let mut infinite = encode_formula(&EffectFormula::empty(), &root);
    infinite[5] = 1;
    let mut bank = meter();
    let prepared = EffectFormula::preflight_source(&infinite, &root, limits(), &mut bank).unwrap();
    assert!(matches!(
        prepared.materialize(&mut bank),
        Err(RuntimeFunctionTypeSourceError::Declaration(
            EffectDeclarationError::InfiniteRow
        ))
    ));
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn effect_argument_source_keeps_parent_prefix_capacity_and_original_work_bank() {
    let (incoming, contract) = fixture();
    let scope = contract.child_scope(&incoming).unwrap();
    let formula = contract.invocation();
    let bytes = encode_formula(formula, &scope);
    let work = 3 * bytes.len() as u64;
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    let prepared = formula
        .prepare_source_encoding(&scope, limits(), &mut bank)
        .unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut output = Vec::with_capacity(bytes.len() + 1);
    output.push(73);
    let pointer = output.as_ptr();
    prepared.append_to(&mut output).unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(&output[1..], bytes);
    let mut output = Vec::with_capacity(bytes.len() - 1);
    output.push(73);
    let prepared = formula
        .prepare_source_encoding(&scope, limits(), &mut meter())
        .unwrap();
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(RuntimeFunctionTypeSourceError::OutputCapacity { .. })
    ));
    assert_eq!(output, [73]);
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    let prepared = EffectFormula::preflight_source(&bytes, &scope, limits(), &mut bank).unwrap();
    prepared.materialize(&mut bank).unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut short = TaskSemanticMeter::new(work - 1, u64::MAX);
    let error = EffectFormula::preflight_source(&bytes, &scope, limits(), &mut short)
        .err()
        .unwrap();
    assert_eq!(
        error,
        RuntimeFunctionTypeSourceError::Resource(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        EffectFormula::preflight_source(&[], &scope, limits(), &mut short)
            .err()
            .unwrap(),
        error
    );
}
