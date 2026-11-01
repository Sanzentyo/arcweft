use super::*;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram},
    engine::{Engine, FlowFiberStatus},
    entry::RuntimeSchemaLimits,
    pattern::RuntimeOpaqueTypeOwner,
    step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions},
    value::{RuntimeBorrowedValue, RuntimeCallTarget, RuntimeRetainedValueAdmission},
};
use arcweft_interaction_model::presentation::PresentationResourceKind;
use arcweft_lang_sema::{
    final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource},
    types::TypeKind,
};

fn nominal_address_fields(native: bool) {
    super::presentation_factories::run_selected(
        super::presentation_factories::program_from_source(
            r#"
struct Addresses { slot: SlotRef<BackgroundSurface>, marker: i32 }
pub fn root() -> (SlotRef<BackgroundSurface>, SlotRef<BackgroundSurface>, i32) {
    let addresses = Addresses { slot: ref.bg(), marker: 42i32 };
    (addresses.slot, addresses.slot, addresses.marker)
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        |value| {
            let RuntimeValue::Tuple(fields) = value else {
                panic!("copied address results");
            };
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0], fields[1]);
            assert!(fields[0].ownership().permits_copy());
            assert_eq!(fields[2], RuntimeValue::i32(42));
        },
    );
}

#[test]
fn source_slot_reference_nominal_fields_copy_in_native() {
    nominal_address_fields(true);
}

#[test]
fn source_slot_reference_nominal_fields_copy_in_decoded_awbc() {
    nominal_address_fields(false);
}

#[test]
fn source_slot_references_execute_as_inert_copied_addresses_in_native_and_decoded_awbc() {
    let compiled = compile_source(r#"
pub character alice { display = "Alice" }
pub type Address<T> = SlotRef<T>
fn identity<T>(value: T) -> T { value }
fn duplicate<T>(value: Address<T>) -> (Address<T>, Address<T>) { (value, value) }
fn score_target(value: Ref<Target>) -> i32 { match value { @target.scene => 1i32, _ => 0i32 } }
fn score_slot(value: Ref<Slot>) -> i32 { match value { @slot.background.far => 1i32, _ => 0i32 } }
pub fn root() -> (Address<BackgroundSurface>, Address<BackgroundSurface>, Address<BackgroundSurface>, Address<CharacterSurface>, Address<CharacterSurface>, Address<CharacterSurface>, Address<BackgroundSurface>, Address<BackgroundSurface>, Address<CharacterSurface>, Address<CharacterSurface>, i32) {
    let (first, second) = duplicate(ref.bg());
    let first = identity::<Address<BackgroundSurface>>(first);
    let target = identity(@target.scene);
    let slot = identity(@slot.background.far);
    let mut order = 0i32;
    let far = ref.bg(slot = scope { order = order * 10i32 + 1i32; slot }, target = scope { order = order * 10i32 + 2i32; target });
    let (alice, again) = duplicate(ref.show(@character.alice));
    let shadow = ref.show(@character.alice, slot = @slot.character.alice.shadow, target = @target.scene);
    let other_background = ref.bg(target = @target.reflection);
    let near = ref.bg(slot = @slot.background.near);
    let other_alice = ref.show(@character.alice, target = @target.reflection);
    let near_alice = ref.show(@character.alice, slot = @slot.character.alice.near);
    (first, second, far, alice, again, shadow, other_background, near, other_alice, near_alice, order + score_target(target) + score_slot(slot) - 2i32)
}
flow main() -> String { return "unused" }
"#).expect("Source slot-reference factories compile");
    let declaration = compiled
        .analysis
        .final_analysis()
        .hir_topology()
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "root")
        .unwrap()
        .declaration()
        .clone();
    let selected = compiled
        .analysis
        .compile_deterministic_program(
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
            }),
            None,
            &Default::default(),
        )
        .unwrap();
    let mut native =
        Engine::for_program_invocation(Arc::clone(selected.plan()), selected.program(), vec![])
            .unwrap();
    let native_owner = RuntimeProgramOwner::Plan(native.program_plan());
    let lowered = AwbcLowerer::new(
        selected.plan(),
        &selected.lowering_report().dialogue_content_catalog,
        "slot-reference.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let encoded = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, lowered);
    let mut invalid = decoded.clone();
    let row = invalid.intrinsics.iter_mut().find(|row| matches!(&row.identity, RuntimeCallTarget::PresentationSlotRef(call) if call.resource() == PresentationResourceKind::Background)).expect("a Source background address constructor is retained");
    let RuntimeCallTarget::PresentationSlotRef(call) = &row.identity else {
        unreachable!()
    };
    row.identity = RuntimeCallTarget::PresentationSlotRef(
        arcweft_core::presentation::RuntimePresentationSlotRefCall::try_new(
            PresentationResourceKind::Character,
            call.inputs().iter().copied(),
        )
        .unwrap(),
    );
    assert!(
        arcweft_core::awbc::product_step::AwbcProductProgram::admit(
            Arc::new(invalid),
            Default::default(),
            Default::default(),
            Default::default(),
            None
        )
        .is_err(),
        "a forged resource/result signature must fail product admission"
    );
    let code = Arc::new(decoded);
    let awbc_owner = RuntimeProgramOwner::Awbc(Arc::clone(&code));
    let mut awbc = AwbcProductStepExecutor::for_program_invocation(
        code,
        selected.program(),
        vec![],
        arcweft_core::task::GenerationId::new(0),
        1,
    )
    .unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    let (mut native_done, mut awbc_done) = (false, false);
    let mut saves = 0;
    for _ in 0..256 {
        if !native_done {
            let step = native.step(Default::default(), options);
            assert!(step.stats.executed_ops <= 1);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            assert!(step.output.requests.tasks.is_empty());
            assert!(step.output.requests.line_commands.is_empty());
            assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
            native_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
        }
        if !awbc_done {
            let step = awbc.step_with_pure_backend(Default::default(), options, &mut backend);
            assert!(step.stats.executed_ops <= 1);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            assert!(step.output.requests.tasks.is_empty());
            assert!(step.output.requests.line_commands.is_empty());
            assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
            awbc_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
            let snapshot = awbc.snapshot_for_save().unwrap();
            let saved =
                arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
                    &snapshot,
                    awbc.admitted_program(),
                )
                .unwrap();
            let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            awbc = awbc
                .restore_inert_snapshot_owned(saved)
                .unwrap_or_else(|(_, error)| {
                    panic!("Source slot-reference public restore: {error}")
                });
            saves += 1;
        }
        if native_done && awbc_done {
            break;
        }
    }
    assert!(native_done && awbc_done);
    assert!(saves > 1);
    for (value, owner) in [
        (
            native.take_program_result().unwrap().unwrap().1,
            native_owner,
        ),
        (awbc.take_program_result().unwrap().unwrap().1, awbc_owner),
    ] {
        assert!(value.ownership().permits_copy());
        let RuntimeValue::Tuple(mut values) = value else {
            panic!("ten actual address values and the source-order observation")
        };
        assert_eq!(values.len(), 11);
        assert_eq!(values.pop(), Some(RuntimeValue::i32(12)));
        let expected = [
            (
                PresentationResourceKind::Background,
                "target.scene",
                "slot.background.default",
            ),
            (
                PresentationResourceKind::Background,
                "target.scene",
                "slot.background.default",
            ),
            (
                PresentationResourceKind::Background,
                "target.scene",
                "slot.background.far",
            ),
            (
                PresentationResourceKind::Character,
                "target.scene",
                "slot.character.alice.default",
            ),
            (
                PresentationResourceKind::Character,
                "target.scene",
                "slot.character.alice.default",
            ),
            (
                PresentationResourceKind::Character,
                "target.scene",
                "slot.character.alice.shadow",
            ),
            (
                PresentationResourceKind::Background,
                "target.reflection",
                "slot.background.default",
            ),
            (
                PresentationResourceKind::Background,
                "target.scene",
                "slot.background.near",
            ),
            (
                PresentationResourceKind::Character,
                "target.reflection",
                "slot.character.alice.default",
            ),
            (
                PresentationResourceKind::Character,
                "target.scene",
                "slot.character.alice.near",
            ),
        ];
        for (value, (resource, target, slot)) in values.iter().zip(expected) {
            let ty = TypeKind::SlotRef(Box::new(TypeKind::PresentationResource(resource)));
            let ty = RuntimeOpaqueTypeOwner::presentation_slot_ref(
                resource,
                ty.semantic_identity_digest().unwrap().into(),
            );
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                RuntimeSchemaLimits::engine_default(),
            );
            let address = admission
                .read_presentation_slot_ref(&ty, RuntimeBorrowedValue::from(value))
                .unwrap();
            assert_eq!(address.resource(), resource);
            assert_eq!(address.target().as_str(), target);
            assert_eq!(address.slot().as_str(), slot);
        }
    }
}
