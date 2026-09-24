use arcweft_character::catalog::{CharacterCatalog, CharacterVisualManifestEvidence};
use arcweft_compiler::source::compile_source;
use arcweft_compiler::types::CompiledSource;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcProgram};
use arcweft_core::awbc::vm::{self, VmError, VmExit, VmHost, VmStepOptions};
use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::pure::{
    RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend,
};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
use arcweft_core::task::RuntimeProgramOwner;
use arcweft_core::value::{RuntimeCallTarget, RuntimeEvalError, RuntimeValue};
use arcweft_dialogue::{
    CharacterDialogueRuntimeExternalCallBackend, CharacterDialogueRuntimeSchema,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

pub(super) fn assert_native_return(source: &str, expected: &str) {
    let compiled = compile_source(source).expect("callable program compiles");
    let [flow] = compiled.plan.flows() else {
        panic!("fixture must contain exactly its main flow");
    };
    let flow = flow.id.clone();
    let mut engine = Engine::for_flow(compiled.plan.clone(), &flow).expect("main flow starts");
    if compiled.character_dialogue_generation.is_some() {
        let schema = bind_character_dialogue_schema(
            &compiled,
            RuntimeProgramOwner::Plan(engine.program_plan()),
        );
        let mut backend = VmRuntimePureCallBackend::default()
            .with_external_calls(CharacterDialogueRuntimeExternalCallBackend::new(&schema));
        assert_native_engine_return(&mut engine, expected, |engine, input, options| {
            engine.step_with_pure_backend(input, options, &mut backend)
        });
    } else {
        assert_native_engine_return(&mut engine, expected, Engine::step);
    }
}

pub(super) type ProducedCharacterDialogues = Vec<(CharacterDialogueOperation, RuntimeValue)>;

pub(super) fn execute_native_character_dialogue_calls(
    compiled: &CompiledSource,
) -> ProducedCharacterDialogues {
    let [flow] = compiled.plan.flows() else {
        panic!("fixture must contain exactly its main flow");
    };
    let flow = flow.id.clone();
    let mut engine = Engine::for_flow(compiled.plan.clone(), &flow).expect("main flow starts");
    let schema =
        bind_character_dialogue_schema(compiled, RuntimeProgramOwner::Plan(engine.program_plan()));
    let produced = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observer = RecordingCharacterDialogueExternalCallBackend {
        dialogue: CharacterDialogueRuntimeExternalCallBackend::new(&schema),
        produced: Arc::clone(&produced),
    };
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(observer);
    for _ in 0..256 {
        let output = engine
            .step_with_pure_backend(
                RuntimeStepInput::default(),
                RuntimeStepOptions::default(),
                &mut backend,
            )
            .output;
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        match &engine.fiber().status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, &FlowExit::Done);
                return produced
                    .lock()
                    .expect("producer observations remain unpoisoned")
                    .clone();
            }
            status => panic!("CharacterDialogue factory flow stopped unexpectedly: {status:?}"),
        }
    }
    panic!("CharacterDialogue factory flow exceeded its deterministic step limit");
}

struct RecordingCharacterDialogueExternalCallBackend<'a> {
    dialogue: CharacterDialogueRuntimeExternalCallBackend<'a>,
    produced: Arc<std::sync::Mutex<ProducedCharacterDialogues>>,
}

impl RuntimeExternalCallBackend for RecordingCharacterDialogueExternalCallBackend<'_> {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        self.dialogue.call_external(context, callee, args)
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.dialogue.produce_character_dialogue(
            owner,
            operation,
            target,
            fields,
            result_type,
        )?;
        self.produced
            .lock()
            .expect("producer observations remain unpoisoned")
            .push((operation, value.clone()));
        Ok(value)
    }
}

fn assert_native_engine_return(
    engine: &mut Engine,
    expected: &str,
    mut step: impl FnMut(
        &mut Engine,
        RuntimeStepInput,
        RuntimeStepOptions,
    ) -> arcweft_core::step::RuntimeStepResult,
) {
    for _ in 0..256 {
        let output = step(
            engine,
            RuntimeStepInput::default(),
            RuntimeStepOptions::default(),
        )
        .output;
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        match &engine.fiber().status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, &FlowExit::Return(expected.to_owned()));
                return;
            }
            status => panic!("callable execution stopped unexpectedly: {status:?}"),
        }
    }
    panic!("callable execution exceeded its deterministic step limit");
}

pub(super) fn assert_awbc_return(source: &str, expected: RuntimeValue) {
    let compiled = compile_source(source).expect("callable program compiles");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "callable_execution.arcw",
    )
    .lower()
    .expect("callable program lowers to verified AWBC");
    let encoded = report
        .program
        .encode_canonical()
        .expect("callable AWBC encodes");
    let program = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("callable AWBC decodes");
    assert_eq!(program, report.program);
    let program = Arc::new(program);
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let schema = compiled
        .character_dialogue_generation
        .as_ref()
        .map(|_| bind_character_dialogue_schema(&compiled, owner));
    let artifact = arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&encoded).as_bytes(),
    )
    .expect("canonical callable program fingerprint");
    let context = vm::VmExecutionContext::for_program(artifact, Arc::clone(&program));
    let mut host = CharacterDialogueVmHost {
        schema: schema.as_ref(),
        produced: Vec::new(),
    };
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).expect("main entry starts");
    for _ in 0..256 {
        let output = vm::step_with_host_context(
            &program,
            &mut fiber,
            VmStepOptions::default(),
            &context,
            &mut host,
        )
        .expect("AWBC callable step succeeds");
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => {
                assert_eq!(value, Some(expected));
                return;
            }
            exit => panic!("AWBC callable execution stopped unexpectedly: {exit:?}"),
        }
    }
    panic!("AWBC callable execution exceeded its deterministic step limit");
}

pub(super) fn execute_decoded_awbc_character_dialogue_calls(
    compiled: &CompiledSource,
) -> ProducedCharacterDialogues {
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "character_dialogue_factory_branches.arcw",
    )
    .lower()
    .expect("CharacterDialogue factory program lowers to verified AWBC");
    let encoded = report
        .program
        .encode_canonical()
        .expect("CharacterDialogue AWBC encodes canonically");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("canonical CharacterDialogue AWBC decodes");
    assert_eq!(decoded, report.program);
    let program = Arc::new(decoded);
    let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
    let schema = compiled
        .character_dialogue_generation
        .as_ref()
        .map(|_| bind_character_dialogue_schema(compiled, owner));
    let artifact = arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&encoded).as_bytes(),
    )
    .expect("canonical CharacterDialogue program fingerprint");
    let context = vm::VmExecutionContext::for_program(artifact, Arc::clone(&program));
    let mut host = CharacterDialogueVmHost {
        schema: schema.as_ref(),
        produced: Vec::new(),
    };
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).expect("main AWBC entry starts");
    for _ in 0..256 {
        let output = vm::step_with_host_context(
            &program,
            &mut fiber,
            VmStepOptions::default(),
            &context,
            &mut host,
        )
        .expect("CharacterDialogue AWBC step succeeds");
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => {
                assert_eq!(value, None);
                return host.produced;
            }
            exit => panic!("CharacterDialogue AWBC stopped unexpectedly: {exit:?}"),
        }
    }
    panic!("CharacterDialogue AWBC exceeded its deterministic step limit");
}

pub(super) fn bind_character_dialogue_schema(
    compiled: &CompiledSource,
    owner: RuntimeProgramOwner,
) -> CharacterDialogueRuntimeSchema {
    let generation = compiled
        .character_dialogue_generation
        .as_ref()
        .expect("the Character factory fixture publishes a generation");
    let environment = compiled.analysis.registered_environment();
    let characters =
        CharacterCatalog::try_from_declarations(generation.characters().keys().map(|character| {
            let visual = environment.character_manifest(character).cloned().map_or(
                CharacterVisualManifestEvidence::Absent,
                CharacterVisualManifestEvidence::Present,
            );
            (character.clone(), visual)
        }))
        .expect("the analysis lease supplies exact accepted Character manifest evidence");

    let product = compiled.dialogue_profile.product();
    let program = product
        .program()
        .expect("the accepted dialogue profile retains its View program");
    let mut views = arcweft_view::ViewRegistry::default();
    program
        .register_runtime_views(&mut views)
        .expect("accepted runtime View registry");
    let style_digest = product.style().map(|style| {
        style
            .resource()
            .canonical_digest()
            .map(|digest| arcweft_core::entry::RuntimeValueDigest::from_bytes(digest.as_bytes()))
            .expect("accepted Style resource digest")
    });

    generation
        .bind_runtime(Arc::new(views), Arc::new(characters), style_digest, owner)
        .expect("CharacterDialogue schema binds to this exact executable generation")
}

struct CharacterDialogueVmHost<'a> {
    schema: Option<&'a CharacterDialogueRuntimeSchema>,
    produced: ProducedCharacterDialogues,
}

impl VmHost for CharacterDialogueVmHost<'_> {
    fn call_intrinsic(
        &mut self,
        _program: &AwbcProgram,
        intrinsic: arcweft_core::awbc::schema::AwbcIntrinsicId,
        _args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        Err(VmError::MissingIntrinsic(intrinsic))
    }

    fn call_pure_helper(
        &mut self,
        _program: &AwbcProgram,
        helper: arcweft_core::awbc::schema::AwbcPureHelperId,
        _args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
        Err(VmError::Runtime(format!(
            "pure helper {} is not bound",
            helper.0
        )))
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, VmError> {
        let Some(schema) = self.schema else {
            return Err(VmError::Runtime(
                "CharacterDialogue producer is not bound to this AWBC generation".to_owned(),
            ));
        };
        let value = schema
            .apply(owner, operation, target, fields, result_type)
            .map_err(|error| VmError::Runtime(error.to_string()))?;
        self.produced.push((operation, value.clone()));
        Ok(value)
    }
}
