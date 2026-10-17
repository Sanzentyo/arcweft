//! Executable wrapper emission from the exact accepted Rust callable proof.

use super::*;
use arcweft_core::plan::RuntimeExprSeedKind;

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
        let target = match program.target() {
            Ok(target) => target,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        let site = match builder.push_function_site_seed(
            program.definition_identity(),
            arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                program.result_type(),
                RuntimeExprSeedKind::Call {
                    callee: target,
                    args: Box::new([]),
                },
            ),
        ) {
            Ok(site) => site,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        if let Err(error) = builder.push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program.program(),
            site,
        }) {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
        }
    }
}
