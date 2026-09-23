//! Executable wrapper emission from the exact accepted Rust callable proof.

use super::*;
use arcweft_core::{
    plan::{RuntimeExprSeedKind, RuntimePureHelperSeed},
    value::RuntimeCallTarget,
};

pub(super) fn lower(
    facts: &RuntimePlanSemanticFacts,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    let mut programs = BTreeMap::new();
    for program in facts.rust_field_default_programs() {
        if let Some(previous) = programs.insert(program.program(), program) {
            if previous != program {
                errors.push(RuntimePlanLowerError::new(
                    "Rust field default program has conflicting checked callable proofs",
                ));
            }
            continue;
        }
        let helper = match builder.push_pure_helper_seed(RuntimePureHelperSeed {
            name: format!("rust.default.{}", program.program()),
            inputs: Box::new([]),
            input_abi: Vec::new(),
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::new(
                program.result_type(),
                RuntimeExprSeedKind::Call {
                    callee: RuntimeCallTarget::callable(program.target()),
                    args: Box::new([]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        }) {
            Ok(helper) => helper,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        if let Err(error) = builder.push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program.program(),
            helper,
        }) {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
        }
    }
}
