//! Typed View value programs and persistent per-mount evaluation state.
//!
//! Arithmetic is delegated to the presentation value evaluator. This module
//! owns View program identity, shared input schemas, dependency revisions,
//! result caching, and exact mount snapshots.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use arcweft_presentation::fx::{
    FxEvaluationBudget, FxEvaluationError, FxRuntimeType, FxRuntimeValue, FxSampleContext,
    ValidatedValueProgram, ValueInstruction, ValueProgramInputs, ValueProgramLimits,
    ValueProgramSchema, ValueProgramValidationError,
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{ViewMountId, ViewProgramId};

/// Dense identifier for one executable View value program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ViewValueProgramId(pub u32);

/// View-owned validated wrapper around the shared value instruction model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewValueProgram {
    id: ViewValueProgramId,
    program: ValidatedValueProgram,
    #[serde(skip)]
    parameter_dependencies: Vec<u16>,
    #[serde(skip)]
    state_dependencies: Vec<u16>,
    #[serde(skip)]
    context_dependent: bool,
}

/// Deterministic inventory whose programs share one mount input schema.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewValueProgramInventory {
    programs: BTreeMap<ViewValueProgramId, ViewValueProgram>,
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
}

/// Whether a requested value was evaluated or reused from the mount cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewValueEvaluationStatus {
    Evaluated,
    Reused,
}

/// One typed value result together with its cache disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewValueEvaluation {
    value: FxRuntimeValue,
    status: ViewValueEvaluationStatus,
}

/// Persisted input slot and its monotonic invalidation revision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewValueSlotSnapshot {
    pub value: FxRuntimeValue,
    pub revision: u64,
}

/// Exact persistent state for one mounted View occurrence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewMountSnapshot {
    pub mount: ViewMountId,
    pub program: ViewProgramId,
    pub state_schema_hash: u64,
    pub parameters: Vec<ViewValueSlotSnapshot>,
    pub state: Vec<ViewValueSlotSnapshot>,
}

/// Reactive input and cache state isolated to one View mount.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMountState {
    mount: ViewMountId,
    program: ViewProgramId,
    state_schema_hash: u64,
    parameters: Vec<ViewValueSlotSnapshot>,
    state: Vec<ViewValueSlotSnapshot>,
    cache: BTreeMap<ViewValueProgramId, CachedViewValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedViewValue {
    parameter_revisions: Vec<u64>,
    state_revisions: Vec<u64>,
    context: Option<FxSampleContext>,
    value: FxRuntimeValue,
}

/// Invalid executable inventory or inconsistent common input schema.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewValueInventoryError {
    #[error("duplicate View value program {program:?}")]
    DuplicateProgram { program: ViewValueProgramId },
    #[error(
        "View value program {program:?} has a different {kind} input schema from the inventory"
    )]
    InputSchemaMismatch {
        program: ViewValueProgramId,
        kind: &'static str,
    },
}

/// Mount creation, update, evaluation, or snapshot validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewValueEvaluationError {
    #[error("unknown View value program {program:?}")]
    UnknownProgram { program: ViewValueProgramId },
    #[error("View mount expected {expected} {kind} values, got {actual}")]
    InputCount {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("View mount {kind} slot {slot} has type {actual:?}, expected {expected:?}")]
    InputType {
        kind: &'static str,
        slot: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("View mount has no {kind} slot {slot}")]
    SlotOutOfBounds { kind: &'static str, slot: u16 },
    #[error("View mount {kind} slot {slot} exhausted its revision counter")]
    RevisionExhausted { kind: &'static str, slot: u16 },
    #[error("View mount snapshot belongs to program {saved:?}, expected {expected:?}")]
    ProgramMismatch {
        saved: ViewProgramId,
        expected: ViewProgramId,
    },
    #[error("View mount snapshot has state schema hash {saved:#018x}, expected {expected:#018x}")]
    StateSchemaMismatch { saved: u64, expected: u64 },
    #[error(transparent)]
    Program(#[from] FxEvaluationError),
}

#[derive(Deserialize)]
struct ViewValueProgramWire {
    id: ViewValueProgramId,
    program: RawValueProgram,
}

#[derive(Deserialize)]
struct RawValueProgram {
    schema: ValueProgramSchema,
    instructions: Vec<ValueInstruction>,
}

impl ViewValueProgram {
    /// Validates a View-owned program under View-specific limits.
    pub fn validate(
        id: ViewValueProgramId,
        schema: ValueProgramSchema,
        instructions: Vec<ValueInstruction>,
    ) -> Result<Self, ValueProgramValidationError> {
        let (parameter_dependencies, state_dependencies, context_dependent) =
            dependencies(&instructions);
        let program =
            ValidatedValueProgram::validate(schema, instructions, ValueProgramLimits::VIEW)?;
        Ok(Self {
            id,
            program,
            parameter_dependencies,
            state_dependencies,
            context_dependent,
        })
    }

    pub const fn id(&self) -> ViewValueProgramId {
        self.id
    }

    pub const fn program(&self) -> &ValidatedValueProgram {
        &self.program
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.program.schema().return_type()
    }

    pub fn parameter_dependencies(&self) -> &[u16] {
        &self.parameter_dependencies
    }

    pub fn state_dependencies(&self) -> &[u16] {
        &self.state_dependencies
    }

    /// Whether evaluation consumes time, ordinal, phase, or motion preference.
    pub const fn is_context_dependent(&self) -> bool {
        self.context_dependent
    }
}

impl<'de> Deserialize<'de> for ViewValueProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ViewValueProgramWire::deserialize(deserializer)?;
        Self::validate(wire.id, wire.program.schema, wire.program.instructions)
            .map_err(D::Error::custom)
    }
}

impl ViewValueProgramInventory {
    pub fn from_programs(
        programs: impl IntoIterator<Item = ViewValueProgram>,
    ) -> Result<Self, ViewValueInventoryError> {
        let mut inventory = Self::default();
        for program in programs {
            inventory.insert(program)?;
        }
        Ok(inventory)
    }

    pub fn insert(&mut self, program: ViewValueProgram) -> Result<(), ViewValueInventoryError> {
        if self.programs.is_empty() {
            self.parameter_types = program.program.schema().parameter_types().to_vec();
            self.state_types = program.program.schema().state_types().to_vec();
        } else {
            if self.parameter_types != program.program.schema().parameter_types() {
                return Err(ViewValueInventoryError::InputSchemaMismatch {
                    program: program.id,
                    kind: "parameter",
                });
            }
            if self.state_types != program.program.schema().state_types() {
                return Err(ViewValueInventoryError::InputSchemaMismatch {
                    program: program.id,
                    kind: "state",
                });
            }
        }
        match self.programs.entry(program.id) {
            Entry::Vacant(entry) => {
                entry.insert(program);
                Ok(())
            }
            Entry::Occupied(_) => Err(ViewValueInventoryError::DuplicateProgram {
                program: program.id,
            }),
        }
    }

    pub fn get(&self, id: ViewValueProgramId) -> Option<&ViewValueProgram> {
        self.programs.get(&id)
    }

    pub fn programs(&self) -> impl ExactSizeIterator<Item = &ViewValueProgram> {
        self.programs.values()
    }

    pub fn parameter_types(&self) -> &[FxRuntimeType] {
        &self.parameter_types
    }

    pub fn state_types(&self) -> &[FxRuntimeType] {
        &self.state_types
    }

    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }
}

impl ViewValueEvaluation {
    pub const fn value(self) -> FxRuntimeValue {
        self.value
    }

    pub const fn status(self) -> ViewValueEvaluationStatus {
        self.status
    }
}

impl ViewMountState {
    pub fn new(
        mount: ViewMountId,
        program: ViewProgramId,
        state_schema_hash: u64,
        parameters: Vec<FxRuntimeValue>,
        state: Vec<FxRuntimeValue>,
        inventory: &ViewValueProgramInventory,
    ) -> Result<Self, ViewValueEvaluationError> {
        validate_inputs("parameter", &parameters, inventory.parameter_types())?;
        validate_inputs("state", &state, inventory.state_types())?;
        Ok(Self {
            mount,
            program,
            state_schema_hash,
            parameters: slots(parameters),
            state: slots(state),
            cache: BTreeMap::new(),
        })
    }

    pub const fn mount(&self) -> ViewMountId {
        self.mount
    }

    pub const fn program(&self) -> ViewProgramId {
        self.program
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub fn parameters(&self) -> impl ExactSizeIterator<Item = FxRuntimeValue> + '_ {
        self.parameters.iter().map(|slot| slot.value)
    }

    pub fn state(&self) -> impl ExactSizeIterator<Item = FxRuntimeValue> + '_ {
        self.state.iter().map(|slot| slot.value)
    }

    pub fn set_parameter(
        &mut self,
        slot: u16,
        value: FxRuntimeValue,
        inventory: &ViewValueProgramInventory,
    ) -> Result<bool, ViewValueEvaluationError> {
        set_slot(
            "parameter",
            slot,
            value,
            &mut self.parameters,
            inventory.parameter_types(),
        )
    }

    pub fn set_state(
        &mut self,
        slot: u16,
        value: FxRuntimeValue,
        inventory: &ViewValueProgramInventory,
    ) -> Result<bool, ViewValueEvaluationError> {
        set_slot(
            "state",
            slot,
            value,
            &mut self.state,
            inventory.state_types(),
        )
    }

    /// Evaluates only when one of this program's consumed slots changed.
    pub fn evaluate(
        &mut self,
        program_id: ViewValueProgramId,
        inventory: &ViewValueProgramInventory,
        context: FxSampleContext,
        budget: &mut FxEvaluationBudget,
    ) -> Result<ViewValueEvaluation, ViewValueEvaluationError> {
        let program =
            inventory
                .get(program_id)
                .ok_or(ViewValueEvaluationError::UnknownProgram {
                    program: program_id,
                })?;
        let parameter_revisions = dependency_revisions(
            "parameter",
            &self.parameters,
            program.parameter_dependencies(),
        )?;
        let state_revisions =
            dependency_revisions("state", &self.state, program.state_dependencies())?;
        let context_key = program.is_context_dependent().then_some(context);
        if let Some(cached) = self.cache.get(&program_id)
            && cached.parameter_revisions == parameter_revisions
            && cached.state_revisions == state_revisions
            && cached.context == context_key
        {
            return Ok(ViewValueEvaluation {
                value: cached.value,
                status: ViewValueEvaluationStatus::Reused,
            });
        }

        let parameters = self.parameters().collect::<Vec<_>>();
        let state = self.state().collect::<Vec<_>>();
        let value = program.program.evaluate(
            ValueProgramInputs {
                parameters: &parameters,
                state: &state,
            },
            context,
            budget,
        )?;
        self.cache.insert(
            program_id,
            CachedViewValue {
                parameter_revisions,
                state_revisions,
                context: context_key,
                value,
            },
        );
        Ok(ViewValueEvaluation {
            value,
            status: ViewValueEvaluationStatus::Evaluated,
        })
    }

    pub fn snapshot(&self) -> ViewMountSnapshot {
        ViewMountSnapshot {
            mount: self.mount,
            program: self.program,
            state_schema_hash: self.state_schema_hash,
            parameters: self.parameters.clone(),
            state: self.state.clone(),
        }
    }

    /// Restores a mount atomically after schema, type, and identity checks.
    pub fn from_snapshot(
        snapshot: &ViewMountSnapshot,
        expected_program: ViewProgramId,
        expected_state_schema_hash: u64,
        inventory: &ViewValueProgramInventory,
    ) -> Result<Self, ViewValueEvaluationError> {
        if snapshot.program != expected_program {
            return Err(ViewValueEvaluationError::ProgramMismatch {
                saved: snapshot.program,
                expected: expected_program,
            });
        }
        if snapshot.state_schema_hash != expected_state_schema_hash {
            return Err(ViewValueEvaluationError::StateSchemaMismatch {
                saved: snapshot.state_schema_hash,
                expected: expected_state_schema_hash,
            });
        }
        let parameters = snapshot
            .parameters
            .iter()
            .map(|slot| slot.value)
            .collect::<Vec<_>>();
        let state = snapshot
            .state
            .iter()
            .map(|slot| slot.value)
            .collect::<Vec<_>>();
        validate_inputs("parameter", &parameters, inventory.parameter_types())?;
        validate_inputs("state", &state, inventory.state_types())?;
        Ok(Self {
            mount: snapshot.mount,
            program: snapshot.program,
            state_schema_hash: snapshot.state_schema_hash,
            parameters: snapshot.parameters.clone(),
            state: snapshot.state.clone(),
            cache: BTreeMap::new(),
        })
    }
}

fn dependencies(instructions: &[ValueInstruction]) -> (Vec<u16>, Vec<u16>, bool) {
    let mut parameters = BTreeSet::new();
    let mut state = BTreeSet::new();
    let mut context_dependent = false;
    for instruction in instructions {
        match instruction {
            ValueInstruction::LoadParameter { slot, .. } => {
                parameters.insert(*slot);
            }
            ValueInstruction::LoadState { slot, .. } => {
                state.insert(*slot);
            }
            ValueInstruction::LoadContext { .. } => context_dependent = true,
            _ => {}
        }
    }
    (
        parameters.into_iter().collect(),
        state.into_iter().collect(),
        context_dependent,
    )
}

fn slots(values: Vec<FxRuntimeValue>) -> Vec<ViewValueSlotSnapshot> {
    values
        .into_iter()
        .map(|value| ViewValueSlotSnapshot { value, revision: 0 })
        .collect()
}

fn validate_inputs(
    kind: &'static str,
    values: &[FxRuntimeValue],
    expected: &[FxRuntimeType],
) -> Result<(), ViewValueEvaluationError> {
    if values.len() != expected.len() {
        return Err(ViewValueEvaluationError::InputCount {
            kind,
            expected: expected.len(),
            actual: values.len(),
        });
    }
    for (slot, (value, expected)) in values.iter().zip(expected).enumerate() {
        let actual = value.value_type();
        if actual != *expected {
            return Err(ViewValueEvaluationError::InputType {
                kind,
                slot,
                expected: *expected,
                actual,
            });
        }
    }
    Ok(())
}

fn set_slot(
    kind: &'static str,
    slot: u16,
    value: FxRuntimeValue,
    values: &mut [ViewValueSlotSnapshot],
    expected: &[FxRuntimeType],
) -> Result<bool, ViewValueEvaluationError> {
    let slot_index = usize::from(slot);
    let expected = expected
        .get(slot_index)
        .copied()
        .ok_or(ViewValueEvaluationError::SlotOutOfBounds { kind, slot })?;
    let actual = value.value_type();
    if actual != expected {
        return Err(ViewValueEvaluationError::InputType {
            kind,
            slot: slot_index,
            expected,
            actual,
        });
    }
    let target = values
        .get_mut(slot_index)
        .ok_or(ViewValueEvaluationError::SlotOutOfBounds { kind, slot })?;
    if target.value == value {
        return Ok(false);
    }
    target.revision = target
        .revision
        .checked_add(1)
        .ok_or(ViewValueEvaluationError::RevisionExhausted { kind, slot })?;
    target.value = value;
    Ok(true)
}

fn dependency_revisions(
    kind: &'static str,
    values: &[ViewValueSlotSnapshot],
    dependencies: &[u16],
) -> Result<Vec<u64>, ViewValueEvaluationError> {
    dependencies
        .iter()
        .map(|slot| {
            values
                .get(usize::from(*slot))
                .map(|value| value.revision)
                .ok_or(ViewValueEvaluationError::SlotOutOfBounds { kind, slot: *slot })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::fx::{
        FxContextSlot, FxEvaluationBudget, FxRuntimeType, FxRuntimeValue, FxSampleContext, Seconds,
        ValueInstruction, ValueProgramSchema,
    };

    use super::{
        ViewMountState, ViewValueEvaluationError, ViewValueEvaluationStatus, ViewValueProgram,
        ViewValueProgramId, ViewValueProgramInventory,
    };
    use crate::{ViewMountAllocator, ViewProgramId};

    fn inventory() -> ViewValueProgramInventory {
        let schema = || {
            ValueProgramSchema::new(
                vec![FxRuntimeType::Bool, FxRuntimeType::I32],
                vec![FxRuntimeType::I32],
                FxRuntimeType::I32,
            )
        };
        ViewValueProgramInventory::from_programs([
            ViewValueProgram::validate(
                ViewValueProgramId(0),
                schema(),
                vec![
                    ValueInstruction::LoadParameter {
                        slot: 1,
                        ty: FxRuntimeType::I32,
                    },
                    ValueInstruction::Return,
                ],
            )
            .unwrap(),
            ViewValueProgram::validate(
                ViewValueProgramId(1),
                schema(),
                vec![
                    ValueInstruction::LoadState {
                        slot: 0,
                        ty: FxRuntimeType::I32,
                    },
                    ValueInstruction::Return,
                ],
            )
            .unwrap(),
        ])
        .unwrap()
    }

    fn context() -> FxSampleContext {
        FxSampleContext::from_elapsed(Seconds::ZERO, 0, 7, false)
    }

    fn context_at(seconds: f32, reduce_motion: bool) -> FxSampleContext {
        FxSampleContext::from_elapsed(Seconds::try_seconds(seconds).unwrap(), 0, 7, reduce_motion)
    }

    fn mount(inventory: &ViewValueProgramInventory) -> ViewMountState {
        let id = ViewMountAllocator::default().allocate().unwrap();
        ViewMountState::new(
            id,
            ViewProgramId(9),
            0xCAFE,
            vec![FxRuntimeValue::Bool(false), FxRuntimeValue::I32(2)],
            vec![FxRuntimeValue::I32(5)],
            inventory,
        )
        .unwrap()
    }

    #[test]
    fn dirty_slots_only_invalidate_programs_that_consume_them() {
        let inventory = inventory();
        let mut mount = mount(&inventory);
        let mut budget = FxEvaluationBudget::default();

        assert_eq!(
            mount
                .evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
                .unwrap()
                .status(),
            ViewValueEvaluationStatus::Evaluated
        );
        assert_eq!(
            mount
                .evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
                .unwrap()
                .status(),
            ViewValueEvaluationStatus::Reused
        );

        mount
            .set_parameter(0, FxRuntimeValue::Bool(true), &inventory)
            .unwrap();
        assert_eq!(
            mount
                .evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
                .unwrap()
                .status(),
            ViewValueEvaluationStatus::Reused
        );

        mount
            .set_parameter(1, FxRuntimeValue::I32(3), &inventory)
            .unwrap();
        let evaluated = mount
            .evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
            .unwrap();
        assert_eq!(evaluated.status(), ViewValueEvaluationStatus::Evaluated);
        assert_eq!(evaluated.value(), FxRuntimeValue::I32(3));
    }

    #[test]
    fn mounts_cache_and_update_values_independently() {
        let inventory = inventory();
        let mut allocator = ViewMountAllocator::default();
        let mut left = ViewMountState::new(
            allocator.allocate().unwrap(),
            ViewProgramId(9),
            1,
            vec![FxRuntimeValue::Bool(false), FxRuntimeValue::I32(2)],
            vec![FxRuntimeValue::I32(5)],
            &inventory,
        )
        .unwrap();
        let mut right = ViewMountState::new(
            allocator.allocate().unwrap(),
            ViewProgramId(9),
            1,
            vec![FxRuntimeValue::Bool(false), FxRuntimeValue::I32(8)],
            vec![FxRuntimeValue::I32(5)],
            &inventory,
        )
        .unwrap();
        let mut budget = FxEvaluationBudget::default();

        assert_ne!(left.mount(), right.mount());
        assert_eq!(
            left.evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
                .unwrap()
                .value(),
            FxRuntimeValue::I32(2)
        );
        assert_eq!(
            right
                .evaluate(ViewValueProgramId(0), &inventory, context(), &mut budget)
                .unwrap()
                .value(),
            FxRuntimeValue::I32(8)
        );
    }

    #[test]
    fn context_dependent_programs_invalidate_on_time_and_motion_changes() {
        let schema = ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::F32);
        let program = ViewValueProgram::validate(
            ViewValueProgramId(2),
            schema,
            vec![
                ValueInstruction::LoadContext {
                    slot: FxContextSlot::Time,
                },
                ValueInstruction::Return,
            ],
        )
        .unwrap();
        assert!(program.is_context_dependent());
        let inventory = ViewValueProgramInventory::from_programs([program]).unwrap();
        let mut mount = ViewMountState::new(
            ViewMountAllocator::default().allocate().unwrap(),
            ViewProgramId(1),
            1,
            Vec::new(),
            Vec::new(),
            &inventory,
        )
        .unwrap();
        let mut budget = FxEvaluationBudget::default();

        assert_eq!(
            mount
                .evaluate(
                    ViewValueProgramId(2),
                    &inventory,
                    context_at(0.0, false),
                    &mut budget,
                )
                .unwrap()
                .status(),
            ViewValueEvaluationStatus::Evaluated
        );
        assert_eq!(
            mount
                .evaluate(
                    ViewValueProgramId(2),
                    &inventory,
                    context_at(0.0, false),
                    &mut budget,
                )
                .unwrap()
                .status(),
            ViewValueEvaluationStatus::Reused
        );
        let advanced = mount
            .evaluate(
                ViewValueProgramId(2),
                &inventory,
                context_at(1.0, false),
                &mut budget,
            )
            .unwrap();
        assert_eq!(advanced.status(), ViewValueEvaluationStatus::Evaluated);
        assert_eq!(
            advanced.value(),
            FxRuntimeValue::F32(arcweft_presentation::fx::FiniteF32::ONE)
        );
        let reduced = mount
            .evaluate(
                ViewValueProgramId(2),
                &inventory,
                context_at(1.0, true),
                &mut budget,
            )
            .unwrap();
        assert_eq!(reduced.status(), ViewValueEvaluationStatus::Evaluated);
        assert_eq!(
            reduced.value(),
            FxRuntimeValue::F32(arcweft_presentation::fx::FiniteF32::ZERO)
        );
    }

    #[test]
    fn snapshot_restore_validates_program_schema_and_preserves_revisions() {
        let inventory = inventory();
        let mut mount = mount(&inventory);
        mount
            .set_state(0, FxRuntimeValue::I32(6), &inventory)
            .unwrap();
        let snapshot = mount.snapshot();
        let restored =
            ViewMountState::from_snapshot(&snapshot, ViewProgramId(9), 0xCAFE, &inventory).unwrap();

        assert_eq!(restored.snapshot(), snapshot);
        assert_eq!(
            ViewMountState::from_snapshot(&snapshot, ViewProgramId(10), 0xCAFE, &inventory),
            Err(ViewValueEvaluationError::ProgramMismatch {
                saved: ViewProgramId(9),
                expected: ViewProgramId(10),
            })
        );
    }

    #[test]
    fn deserialization_revalidates_program_stack_and_limits() {
        let invalid = r#"{
            "id": 3,
            "program": {
                "schema": {"parameter_types": [], "state_types": [], "return_type": "bool"},
                "instructions": [{"op": "return"}]
            }
        }"#;

        assert!(serde_json::from_str::<ViewValueProgram>(invalid).is_err());
    }
}
