use super::source_declarations::{
    declaration_candidate, define_declaration_body, define_scoped_declaration_body,
    nominal_candidate, scoped_declaration_candidate,
};
use super::*;
use crate::plan::construction::source::RuntimePlanSourceConstruction;
use crate::plan::declaration_sources::{
    PreparedRuntimePlanDeclarationSources, RuntimePlanDeclarationSourceBytes,
    RuntimePlanDeclarationSourceError, RuntimePlanDeclarationSourceLimits,
};
use crate::plan::{
    RuntimeControlEffectContractDefinition, RuntimeControlEffectContractError, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSemanticRole,
    RuntimeLocalReadSeed, RuntimePlanBuildError, RuntimePlanBuilder, RuntimeTaskControlMode,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::value::{RuntimeLocalReadMode, RuntimeValue};

fn source_limits() -> RuntimePlanDeclarationSourceLimits {
    RuntimePlanDeclarationSourceLimits {
        types: 100,
        nominals: 100,
        locals: 100,
        controls: 100,
        elements: 100_000,
        encoded_bytes: 100_000,
        decoded_bytes: 100_000,
        policy_depth: 32,
    }
}

fn preflight<'a>(
    frames: &'a [Vec<u8>; 4],
    limits: RuntimePlanDeclarationSourceLimits,
    meter: &mut TaskSemanticMeter,
) -> Result<PreparedRuntimePlanDeclarationSources<'a>, RuntimePlanDeclarationSourceError> {
    RuntimePlanDeclarationSourceBytes {
        types: &frames[0],
        nominals: &frames[1],
        locals: &frames[2],
        controls: &frames[3],
    }
    .preflight(limits, meter)
}

fn construction(frames: &[Vec<u8>; 4]) -> RuntimePlanSourceConstruction {
    let mut meter = TaskSemanticMeter::new(1_000_000, 1_000_000);
    preflight(frames, source_limits(), &mut meter)
        .unwrap()
        .into_construction(meter)
        .unwrap()
}

fn finish(source: RuntimePlanSourceConstruction) -> Result<RuntimePlan, RuntimeTaskPlanSealError> {
    let limits = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(limits)?
        .seal(limits, None)
        .map(|sealed| sealed.into_plan())
}

fn first_local() -> RuntimeLocalDeclarationId {
    RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN)
}

#[test]
fn source_declarations_enter_the_actual_builder_and_preserve_local_coordinates_and_common_seal() {
    for (candidate, define) in [
        (
            declaration_candidate as fn() -> UnsealedRuntimePlanImage,
            define_declaration_body as fn(&mut RuntimePlanBuilder, crate::plan::RuntimeLocalSeedId),
        ),
        (scoped_declaration_candidate, define_scoped_declaration_body),
    ] {
        let original = candidate()
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .unwrap()
            .into_plan();
        let frames = declaration_frames(&original);
        let mut source = construction(&frames);
        let local = source.local(first_local()).unwrap().clone();
        define(&mut source.builder, local.clone());
        let restored = finish(source).unwrap();
        // The source handle was bound to the existing row, even across clones
        // and repeated uses in a declaration, pattern and expression.
        assert_eq!(restored.local_declarations().len(), 1);
        assert_eq!(restored.local_declarations(), original.local_declarations());
        assert_eq!(restored.type_table(), original.type_table());
        assert_eq!(
            restored.control_effect_contracts(),
            original.control_effect_contracts()
        );
        assert_eq!(
            restored.executable_semantic_digest(),
            original.executable_semantic_digest()
        );
        assert_eq!(declaration_frames(&restored), frames);
    }
}

#[test]
fn source_nominal_policies_and_control_definitions_reach_the_actual_builder_without_finished_proofs()
 {
    let original = nominal_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let mut source = construction(&declaration_frames(&original));
    let boolean = crate::pattern::RuntimeSemanticTypeId::from_bytes([3; 32]);
    source
        .builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([125; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ),
        )
        .unwrap();
    let restored = finish(source).unwrap();
    assert_eq!(
        restored.nominal_record_domains(),
        original.nominal_record_domains()
    );
    assert_eq!(restored.variant_domains(), original.variant_domains());
    assert_eq!(restored.type_table(), original.type_table());
    assert_eq!(
        restored.executable_semantic_digest(),
        original.executable_semantic_digest()
    );
}

#[test]
fn source_local_and_control_handles_keep_their_actual_builder_issuer() {
    let original = declaration_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let frames = declaration_frames(&original);
    let first = construction(&frames);
    let mut second = construction(&frames);
    let mut rejecting = construction(&frames);
    let unit = crate::pattern::RuntimeSemanticTypeId::from_bytes([101; 32]);
    assert!(matches!(
        rejecting.builder.push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([120; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                unit,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    first.local(first_local()).unwrap().clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            ),
        ),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    ));
    let control = RuntimeControlEffectContractId::for_index(0).unwrap();
    assert!(matches!(
        second
            .builder
            .push_control_effect_contract(RuntimeControlEffectContractDefinition {
                mode: RuntimeTaskControlMode::StraightLine,
                effects: Box::new([]),
                children: Box::new([first.control(control).unwrap().clone()]),
            }),
        Err(RuntimePlanBuildError::ControlEffectContract(
            RuntimeControlEffectContractError::ForeignSeed
        ))
    ));
    let local = second.local(first_local()).unwrap().clone();
    define_declaration_body(&mut second.builder, local);
    let restored = finish(second).unwrap();
    assert_eq!(
        restored.executable_semantic_digest(),
        original.executable_semantic_digest()
    );
}

#[test]
fn declaration_source_handles_supply_no_local_placement_or_control_completion_proof() {
    let original = declaration_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let mut frames = declaration_frames(&original);
    assert!(matches!(
        finish(construction(&frames)),
        Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(*error,
            RuntimePlanBuildError::LocalPlacement(crate::plan::RuntimeLocalPlacementError::Unowned { .. }))
    ));
    // A structurally valid source may contain a forward/self control reference;
    // only the common executable seal can prove that the whole graph is acyclic.
    let last = frames[3].len() - 4;
    frames[3][last..].copy_from_slice(&1u32.to_le_bytes());
    frames[3].extend_from_slice(&0u32.to_le_bytes()); // zero-based source child ordinal
    frames[3].extend_from_slice(&1u32.to_le_bytes()); // original control coordinate
    let mut source = construction(&frames);
    let local = source.local(first_local()).unwrap().clone();
    define_declaration_body(&mut source.builder, local);
    let image = source
        .prepare_image(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    assert!(matches!(
        image.seal(RuntimeTaskPlanSealLimits::default(), None),
        Err(RuntimeTaskPlanSealError::Control(
            RuntimeControlEffectContractError::Cycle { index: 0 }
        ))
    ));
}

#[test]
fn declaration_construction_checks_the_combined_handle_storage_and_original_sticky_bank_before_retention()
 {
    let original = declaration_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let frames = declaration_frames(&original);
    let mut meter = TaskSemanticMeter::new(1_000_000, 1_000_000);
    let prepared = preflight(&frames, source_limits(), &mut meter).unwrap();
    let storage = prepared.construction_storage_bytes().unwrap();
    assert!(storage > prepared.storage_bytes());
    for maximum in [storage, storage - 1] {
        let mut bank = TaskSemanticMeter::new(1_000_000, 1_000_000);
        let prepared = preflight(
            &frames,
            RuntimePlanDeclarationSourceLimits {
                decoded_bytes: maximum,
                ..source_limits()
            },
            &mut bank,
        )
        .unwrap();
        let result = prepared.into_construction(bank);
        if maximum == storage {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(result, Err(RuntimePlanDeclarationSourceError::Limit {
                kind: "decoded bytes", actual, maximum,
            }) if actual == storage && maximum == storage - 1)
            );
        }
    }
    // A source decoder cannot reset the bank that its caller already exhausted.
    let mut bank = TaskSemanticMeter::new(1_000_000, 1_000_000);
    let prepared = preflight(&frames, source_limits(), &mut bank).unwrap();
    let error = bank.constrain(0, 1_000_000).unwrap_err();
    assert!(matches!(prepared.into_construction(bank),
        Err(RuntimePlanDeclarationSourceError::Resource(cause)) if cause == error));
    // Malformed final definitions reject the combined preparation itself.
    let mut malformed = frames;
    malformed[3].pop();
    let mut bank = TaskSemanticMeter::new(1_000_000, 1_000_000);
    assert!(matches!(
        preflight(&malformed, source_limits(), &mut bank),
        Err(RuntimePlanDeclarationSourceError::Controls(_))
    ));
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn declaration_source_work_bank_moves_into_the_common_image_without_a_new_allowance() {
    let original = RuntimePlanBuilder::new().finish().unwrap();
    let frames = declaration_frames(&original);
    let source = construction(&frames);
    let limited = RuntimeTaskPlanSealLimits {
        max_semantic_work: 16,
        ..RuntimeTaskPlanSealLimits::default()
    };
    // A fresh empty authoring builder can prepare within this allowance. Its
    // source-restored counterpart already spent more on the four source frames.
    assert!(
        RuntimePlanBuilder::new()
            .prepare_task_image(limited)
            .is_ok()
    );
    assert!(matches!(
        source.prepare_image(limited),
        Err(RuntimeTaskPlanSealError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let source = construction(&frames);
    assert_eq!(
        finish(source).unwrap().executable_semantic_digest(),
        original.executable_semantic_digest()
    );
}
