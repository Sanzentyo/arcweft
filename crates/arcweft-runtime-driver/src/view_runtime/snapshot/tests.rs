use super::*;
use arcweft_core::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use arcweft_core::value::RuntimeValue;
use std::sync::Arc;

fn empty_snapshot() -> BundleViewRuntimeSnapshot {
    BundleViewRuntimeSnapshot {
        program_id: None,
        logical_time: arcweft_presentation::fx::FxLogicalTime::zero(),
        next_mount_id: 0,
        view_root_bindings: vec![],
        mounts: vec![],
        axis_seeds: super::super::BundleViewAxisSeedRegistrySnapshot::default(),
    }
}

#[test]
fn view_snapshot_metadata_preflight_precedes_earlier_payload_rejection() {
    let limits = RuntimeSchemaLimits::engine_default();
    let long = "x".repeat(usize::try_from(limits.max_string_bytes).unwrap() + 1);
    let mut live = empty_snapshot();
    live.view_root_bindings = vec![
        RuntimeBinding {
            name: "earlier".into(),
            value: RuntimeValue::F64(f64::NAN),
        },
        RuntimeBinding {
            name: long.clone(),
            value: RuntimeValue::Unit,
        },
    ];
    let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
    assert_eq!(
        live.admit_resources(&mut admission),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes"
            }
        ))
    );

    let owner =
        RuntimeProgramOwner::Awbc(Arc::new(arcweft_core::awbc::schema::AwbcProgram::default()));
    let mut inert = BundleViewRuntimeSaveSnapshot::from_live(&empty_snapshot(), &owner).unwrap();
    inert.view_root_bindings = vec![
        BindingSnapshot {
            name: "earlier".into(),
            value: AwbcRuntimeValueSnapshot::F64(f64::NAN.to_bits()),
        },
        BindingSnapshot {
            name: long,
            value: AwbcRuntimeValueSnapshot::Unit,
        },
    ];
    let error = inert.into_live(&owner).unwrap_err();
    assert!(error.contains("string_bytes"), "{error}");
}

#[test]
fn view_snapshot_tables_are_bounded_before_inert_activation() {
    let owner =
        RuntimeProgramOwner::Awbc(Arc::new(arcweft_core::awbc::schema::AwbcProgram::default()));
    let mut inert = BundleViewRuntimeSaveSnapshot::from_live(&empty_snapshot(), &owner).unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    inert.view_root_bindings = (0..=limits.max_sequence_items)
        .map(|_| BindingSnapshot {
            name: String::new(),
            value: AwbcRuntimeValueSnapshot::Unit,
        })
        .collect();
    let error = inert.into_live(&owner).unwrap_err();
    assert!(error.contains("sequence_items"), "{error}");
}
