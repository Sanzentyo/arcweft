use super::{bind_character_dialogue_schema, compile_source};
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcEntryId, AwbcProgram},
    },
    engine::{Engine, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot},
    pattern::RuntimeSemanticTypeId,
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    step::{RuntimeStepInput, RuntimeStepOptions},
    task::RuntimeProgramOwner,
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeValue},
};
use arcweft_dialogue::{
    CharacterDialogueRichTextProperties, CharacterDialogueRichTextPropertyValue as PropertyValue,
    CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeExternalCallBackend,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField,
};
use arcweft_presentation::rich_text::PresentationReusableStylePropertyId as Property;
use arcweft_rich_text_schema::RichTextUnit;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::{Arc, Mutex};

struct Observe<'a> {
    backend: CharacterDialogueRuntimeExternalCallBackend<'a>,
    values: Arc<Mutex<Vec<CharacterDialogueRichTextProperties>>>,
}

impl RuntimeExternalCallBackend for Observe<'_> {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let result = self.backend.call_external(context, callee, args)?;
        if let Ok(RuntimeValue::Opaque(value)) = &result {
            self.values.lock().unwrap().push(
                CharacterDialogueRolePayloadCodec::RichTextProperties
                    .decode_properties(value.payload())
                    .unwrap(),
            );
        }
        Some(result)
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.backend
            .produce_character_dialogue(owner, operation, target, fields, result_type)
    }
}

fn execute(body: &str) -> [Vec<CharacterDialogueRichTextProperties>; 2] {
    let source = format!(
        "pub character alice {{ display = \"Alice\" }}\nflow main() {{ {body}\nlet configured = alice(rich_text = settings)\n}}\nentry cli @entry.main {{ goto @flow.main }}"
    );
    let compiled = compile_source(&source)
        .unwrap_or_else(|error| panic!("factory source compiles: {error:?}"));
    let [flow] = compiled.plan.flows() else {
        panic!("one flow")
    };
    let mut native = Engine::for_flow(compiled.plan.clone(), &flow.id).unwrap();
    let native_schema =
        bind_character_dialogue_schema(&compiled, RuntimeProgramOwner::Plan(native.program_plan()));
    let native_values = Arc::new(Mutex::new(Vec::new()));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(Observe {
        backend: CharacterDialogueRuntimeExternalCallBackend::new(&native_schema),
        values: Arc::clone(&native_values),
    });
    let mut done = false;
    for _ in 0..512 {
        let result = native.step_with_pure_backend(
            RuntimeStepInput::default(),
            RuntimeStepOptions::default(),
            &mut backend,
        );
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        if matches!(result.fiber_status, FlowFiberStatus::Done(_)) {
            done = true;
            break;
        }
    }
    assert!(
        done,
        "native factory flow terminates within its fixed step budget"
    );
    let report = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "factory.arcw")
        .lower()
        .unwrap();
    let decoded = AwbcProgram::decode_canonical(
        &report.program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(decoded, report.program);
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
    let awbc_schema = bind_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Awbc(
            executor
                .admitted_product_awbc_program()
                .unwrap()
                .program_arc(),
        ),
    );
    let awbc_values = Arc::new(Mutex::new(Vec::new()));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(Observe {
        backend: CharacterDialogueRuntimeExternalCallBackend::new(&awbc_schema),
        values: Arc::clone(&awbc_values),
    });
    let mut done = false;
    for _ in 0..512 {
        let result = executor.step_with_pure_backend(
            RuntimeStepInput::default(),
            RuntimeStepOptions::default(),
            &mut backend,
        );
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(saved) = executor.snapshot().unwrap();
        let saved = arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
            &saved,
            executor.admitted_product_awbc_program().unwrap(),
        )
        .unwrap();
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let saved = ArcweftRuntimeExecutorSnapshot::AwbcProduct(
            executor.bind_product_save_snapshot(saved).unwrap(),
        );
        executor = executor.restore_snapshot(saved).unwrap();
        if matches!(result.fiber_status, FlowFiberStatus::Done(_)) {
            done = true;
            break;
        }
    }
    assert!(
        done,
        "decoded AWBC factory flow terminates within its fixed step budget"
    );
    let native = native_values.lock().unwrap().clone();
    let awbc = awbc_values.lock().unwrap().clone();
    assert_eq!(native, awbc);
    [native, awbc]
}

#[test]
fn source_style_factories_keep_empty_optional_aliases_in_native_and_restored_awbc() {
    for expression in [
        "text_style()",
        "ruby_style()",
        "rich_text_style()",
        "{ let make = text_style; make() }",
        "{ let make = ruby_style; make() }",
        "{ let make = rich_text_style; make() }",
    ] {
        for values in execute(&format!("let settings = {expression}")) {
            assert_eq!(values.len(), 1);
            assert!(values[0].is_empty());
        }
    }
}

#[test]
fn source_style_factories_preserve_typography_values_and_units_during_composition() {
    for values in execute(
        "let settings = rich_text_style(text = text_style(font = \"Noto Sans JP\", color = rgb(\"#123456\"), size = 24px), ruby = ruby_style(size = 11pt, gap = 1px))",
    ) {
        assert_eq!(values.len(), 3);
        let final_value = values.last().unwrap();
        assert_eq!(final_value.len(), 5);
        for (property, milli, unit) in [
            (Property::Size, 24_000, RichTextUnit::Px),
            (Property::RubySize, 11_000, RichTextUnit::Pt),
            (Property::RubyGap, 1_000, RichTextUnit::Px),
        ] {
            assert_eq!(
                final_value
                    .iter()
                    .find(|field| field.id() == property)
                    .unwrap()
                    .value(),
                &PropertyValue::Length { milli, unit }
            );
        }
    }
}

#[test]
fn source_style_factories_use_caller_order_for_conflicting_properties() {
    for body in [
        "let settings = rich_text_style(ruby = text_style(size = 11px), text = text_style(size = 24px))",
        "let compose = rich_text_style; let settings = compose(ruby = text_style(size = 11px), text = text_style(size = 24px))",
    ] {
        for values in execute(body) {
            assert_eq!(values.len(), 3);
            assert_eq!(
                values
                    .last()
                    .unwrap()
                    .iter()
                    .find(|field| field.id() == Property::Size)
                    .unwrap()
                    .value(),
                &PropertyValue::Length {
                    milli: 24_000,
                    unit: RichTextUnit::Px
                },
                "the last authored property wins for both direct and alias calls",
            );
        }
    }
}
