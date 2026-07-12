//! Deterministic execution state for bundle-authored View programs.
//!
//! A View definition may be mounted by more than one presentation handle and
//! may recursively mount child definitions. This module owns those occurrence
//! identities, persistent typed value slots, activation-relative logical time,
//! exact save state, and renderer-neutral frame output.

mod evaluator;
mod value;

use crate::presentation_handles::PresentationHandleId;
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::resource_codec::view::{ViewObserveClassification, ViewTextSelectionPolicy};
use arcweft_bundle::resource_codec::{
    ViewDefinitionResource, ViewProgramResource, ViewRuntimeControlStyle,
    ViewRuntimeControlStyleDiagnostics, ViewStyleResource, ViewTextBlockBounds, ViewTextResource,
};
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use arcweft_presentation::fx::{
    FiniteF32Error, FxGraphChildPath, FxId, FxInstanceId, FxLogicalTime, FxRuntimeType,
    FxRuntimeValue,
};
use arcweft_render_text::{LineDisplayFrame, RichTextDocument};
use arcweft_view::{
    ViewMountAllocationError, ViewMountAllocator, ViewMountId, ViewMountSnapshot, ViewMountState,
    ViewProgramId, ViewValueEvaluationError, ViewValueInventoryError, ViewValueProgramInventory,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;

pub use value::BundleViewValueConversionError;

const MAX_VIEW_INSTANCE_PATH_DEPTH: usize = 63;

/// Stable structural path of one mounted definition below a presentation handle.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct BundleViewInstancePath(Vec<BundleViewInstancePathSegment>);

/// One authored structural identity component in a mounted View graph.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleViewInstancePathSegment {
    Call {
        instruction: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authored_key: Option<u64>,
    },
    Repeat {
        instruction: u32,
        key: i32,
    },
}

/// Machine-readable category for one non-fatal View frame failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleViewDiagnosticCode {
    MissingDefinition,
    MissingInput,
    InputType,
    InvalidValueProgram,
    EvaluationBudgetExceeded,
    InvalidControlFlow,
    RepeatLimitExceeded,
    DuplicateRepeatKey,
    RecursionLimitExceeded,
    MissingTextSource,
    UnsupportedTextValue,
    InvalidAwaitState,
    InvalidFxApplication,
    MissingLocalizedText,
    MissingRichTextDocument,
    MissingDisplayFrame,
    InvalidDisplayStage,
}

impl BundleViewDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingDefinition => "VIEW001_MISSING_DEFINITION",
            Self::MissingInput => "VIEW002_MISSING_INPUT",
            Self::InputType => "VIEW003_INPUT_TYPE",
            Self::InvalidValueProgram => "VIEW004_INVALID_VALUE_PROGRAM",
            Self::EvaluationBudgetExceeded => "VIEW005_EVALUATION_BUDGET_EXCEEDED",
            Self::InvalidControlFlow => "VIEW006_INVALID_CONTROL_FLOW",
            Self::RepeatLimitExceeded => "VIEW007_REPEAT_LIMIT_EXCEEDED",
            Self::DuplicateRepeatKey => "VIEW008_DUPLICATE_REPEAT_KEY",
            Self::RecursionLimitExceeded => "VIEW009_RECURSION_LIMIT_EXCEEDED",
            Self::MissingTextSource => "VIEW010_MISSING_TEXT_SOURCE",
            Self::UnsupportedTextValue => "VIEW011_UNSUPPORTED_TEXT_VALUE",
            Self::InvalidAwaitState => "VIEW012_INVALID_AWAIT_STATE",
            Self::InvalidFxApplication => "VIEW013_INVALID_FX_APPLICATION",
            Self::MissingLocalizedText => "VIEW014_MISSING_LOCALIZED_TEXT",
            Self::MissingRichTextDocument => "VIEW015_MISSING_RICH_TEXT_DOCUMENT",
            Self::MissingDisplayFrame => "VIEW016_MISSING_DISPLAY_FRAME",
            Self::InvalidDisplayStage => "VIEW017_INVALID_DISPLAY_STAGE",
        }
    }
}

/// Typed diagnostic shared by native, Web, headless, and observation adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewDiagnostic {
    pub code: BundleViewDiagnosticCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<PresentationHandleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<ViewMountId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<u32>,
    pub message: String,
}

impl fmt::Display for BundleViewDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

/// Typed text payload retained until the shared text-preparation boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleViewTextValue {
    Plain {
        value: String,
    },
    Localized {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
        document: Box<RichTextDocument>,
    },
    RichTextDocument {
        document: Box<RichTextDocument>,
    },
    DisplayFrame {
        frame: Box<LineDisplayFrame>,
        stage_index: u32,
    },
}

/// One authored text target retained with its typed layout and style contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewTextTarget {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub bounds: ViewTextBlockBounds,
    #[serde(default)]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default, skip_serializing_if = "ViewRuntimeControlStyle::is_default")]
    pub style: ViewRuntimeControlStyle,
}

/// One active text source in a concrete mounted occurrence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleViewTextOutput {
    pub source_id: String,
    pub targets: Vec<BundleViewTextTarget>,
    pub value: BundleViewTextValue,
    #[serde(default)]
    pub classification: ViewObserveClassification,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

/// One evaluated named Fx argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewFxArgument {
    pub parameter: String,
    pub value: FxRuntimeValue,
}

/// One active View-side Fx application ready for shared Fx reconciliation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewFxApplication {
    pub instance: FxInstanceId,
    pub definition: FxId,
    pub target: String,
    pub application_ordinal: u32,
    pub arguments: Vec<BundleViewFxArgument>,
    pub child_path: FxGraphChildPath,
}

/// One per-mount View paint operation in exact evaluator order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleViewPaintItem {
    Element { target: String },
    Text { source_id: String, target: String },
    Image { target: String },
    Mount { mount: ViewMountId },
}

/// Renderer-neutral output of one concrete View definition occurrence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleViewMountOutput {
    pub handle: PresentationHandleId,
    pub mount: ViewMountId,
    pub view: String,
    pub path: BundleViewInstancePath,
    pub active_targets: Vec<String>,
    pub active_images: Vec<String>,
    pub paint: Vec<BundleViewPaintItem>,
    pub text: Vec<BundleViewTextOutput>,
    pub fx: Vec<BundleViewFxApplication>,
}

impl BundleViewMountOutput {
    /// Qualifies one authored resource identity for this concrete mount occurrence.
    #[must_use]
    pub fn scoped_id(&self, authored: &str) -> String {
        format!("view_mount_{}.{}", self.mount.get(), authored)
    }
}

/// Complete result of one deterministic View evaluation frame.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BundleViewFrame {
    pub mounts: Vec<BundleViewMountOutput>,
    pub diagnostics: Vec<BundleViewDiagnostic>,
}

impl BundleViewFrame {
    /// Removes masked and secret text while preserving structural observation data.
    #[must_use]
    pub fn redacted_for_observation(&self) -> Self {
        let mut redacted = self.clone();
        for mount in &mut redacted.mounts {
            for text in &mut mount.text {
                let replacement = match text.classification {
                    ViewObserveClassification::Public => continue,
                    ViewObserveClassification::AgentMasked => text
                        .replacement
                        .clone()
                        .unwrap_or_else(|| "••••".to_owned()),
                    ViewObserveClassification::Secret => {
                        text.replacement.clone().unwrap_or_default()
                    }
                };
                text.value = BundleViewTextValue::Plain { value: replacement };
            }
        }
        redacted
    }
}

/// Exact persisted state for the View runtime owned by one bundle session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleViewRuntimeSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    pub logical_time: FxLogicalTime,
    pub next_mount_id: u64,
    pub root_bindings: Vec<RuntimeBinding>,
    pub mounts: Vec<BundleViewMountRuntimeSnapshot>,
}

/// Exact persisted state for one root or nested mounted View occurrence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleViewMountRuntimeSnapshot {
    pub handle: PresentationHandleId,
    pub path: BundleViewInstancePath,
    pub definition: String,
    pub activation_logical_time: FxLogicalTime,
    pub deterministic_seed: u64,
    pub state: ViewMountSnapshot,
    pub initialized_parameters: Vec<u16>,
    pub initialized_state: Vec<u16>,
    pub runtime_parameters: Vec<RuntimeBinding>,
}

/// Fatal construction or snapshot restoration failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum BundleViewRuntimeError {
    #[error(transparent)]
    Inventory(#[from] ViewValueInventoryError),
    #[error(transparent)]
    MountAllocation(#[from] ViewMountAllocationError),
    #[error(transparent)]
    MountState(#[from] ViewValueEvaluationError),
    #[error(transparent)]
    LogicalTime(#[from] FiniteF32Error),
    #[error("View program repeats definition `{definition}`")]
    DuplicateDefinition { definition: String },
    #[error("View definition `{definition}` has an invalid instruction span")]
    InvalidDefinitionSpan { definition: String },
    #[error("View snapshot belongs to program {saved:?}, expected {expected:?}")]
    ProgramMismatch {
        saved: Option<String>,
        expected: Option<String>,
    },
    #[error("View snapshot repeats occurrence `{handle}` at path {path:?}")]
    DuplicateOccurrence {
        handle: PresentationHandleId,
        path: BundleViewInstancePath,
    },
    #[error("View snapshot repeats mount id {mount:?}")]
    DuplicateMount { mount: ViewMountId },
    #[error("View mount {mount:?} was not issued below shared allocator cursor {next}")]
    UnallocatedMount { mount: ViewMountId, next: u64 },
    #[error("View snapshot references unknown definition `{definition}`")]
    UnknownDefinition { definition: String },
    #[error("View snapshot path exceeds the maximum depth of {limit}")]
    InstancePathTooDeep { limit: usize },
    #[error("View snapshot mount activates after the saved logical time")]
    ActivationAfterRuntime,
    #[error("View snapshot has invalid initialized {kind} slot {slot}")]
    InvalidInitializedSlot { kind: &'static str, slot: u16 },
    #[error("View snapshot repeats runtime parameter `{parameter}`")]
    DuplicateRuntimeParameter { parameter: String },
    #[error("View snapshot repeats root binding `{binding}`")]
    DuplicateRootBinding { binding: String },
    #[error("saved View presentation frame does not match the retained mount table: {message}")]
    PresentationFrameMismatch { message: String },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ViewOccurrenceKey {
    handle: PresentationHandleId,
    path: BundleViewInstancePath,
}

#[derive(Clone, Debug, PartialEq)]
struct MountedView {
    definition: String,
    activation_logical_time: FxLogicalTime,
    deterministic_seed: u64,
    state: ViewMountState,
    initialized_parameters: BTreeSet<u16>,
    initialized_state: BTreeSet<u16>,
    runtime_parameters: BTreeMap<String, RuntimeValue>,
}

/// Sans I/O evaluator and persistent mount table for one active View program.
#[derive(Clone, Debug)]
pub struct BundleViewRuntime {
    program: Option<ViewProgramResource>,
    text: Option<ViewTextResource>,
    text_styles: BTreeMap<String, ViewRuntimeControlStyle>,
    text_style_diagnostics: ViewRuntimeControlStyleDiagnostics,
    definitions: BTreeMap<String, usize>,
    inventory: ViewValueProgramInventory,
    logical_time: FxLogicalTime,
    allocator: ViewMountAllocator,
    root_bindings: BTreeMap<String, RuntimeValue>,
    mounts: BTreeMap<ViewOccurrenceKey, MountedView>,
}

impl Default for BundleViewRuntime {
    fn default() -> Self {
        Self::try_new(None, None, None).expect("an empty View runtime is valid")
    }
}

impl BundleViewInstancePath {
    #[must_use]
    pub fn segments(&self) -> &[BundleViewInstancePathSegment] {
        &self.0
    }

    fn validate(&self) -> Result<(), BundleViewRuntimeError> {
        if self.0.len() > MAX_VIEW_INSTANCE_PATH_DEPTH {
            Err(BundleViewRuntimeError::InstancePathTooDeep {
                limit: MAX_VIEW_INSTANCE_PATH_DEPTH,
            })
        } else {
            Ok(())
        }
    }

    fn with_segment(
        &self,
        segment: BundleViewInstancePathSegment,
    ) -> Result<Self, BundleViewRuntimeError> {
        let mut segments = self.0.clone();
        segments.push(segment);
        let path = Self(segments);
        path.validate()?;
        Ok(path)
    }
}

impl BundleViewRuntime {
    /// Builds an evaluator from already-decoded, typed bundle resources.
    pub fn try_new(
        program: Option<ViewProgramResource>,
        text: Option<ViewTextResource>,
        style: Option<&ViewStyleResource>,
    ) -> Result<Self, BundleViewRuntimeError> {
        let inventory = ViewValueProgramInventory::from_programs(
            program
                .as_ref()
                .map_or_else(Vec::new, |program| program.value_programs.clone()),
        )?;
        let mut definitions = BTreeMap::new();
        if let Some(program) = &program {
            for (index, definition) in program.definitions.iter().enumerate() {
                if definitions
                    .insert(definition.public_id.clone(), index)
                    .is_some()
                {
                    return Err(BundleViewRuntimeError::DuplicateDefinition {
                        definition: definition.public_id.clone(),
                    });
                }
                validate_definition_span(definition, program.instructions.len())?;
                let _ = definition_program_id(index)?;
            }
        }
        let styled_text = program.as_ref().map_or_else(Default::default, |program| {
            program.runtime_text_styles_with_style(style)
        });
        let text_styles = styled_text
            .controls
            .into_iter()
            .map(|binding| (binding.public_id, binding.style))
            .collect();
        Ok(Self {
            program,
            text,
            text_styles,
            text_style_diagnostics: styled_text.diagnostics,
            definitions,
            inventory,
            logical_time: FxLogicalTime::zero(),
            allocator: ViewMountAllocator::default(),
            root_bindings: BTreeMap::new(),
            mounts: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn logical_time(&self) -> FxLogicalTime {
        self.logical_time
    }

    /// Shared allocator for authored and Rust-backed retained View mounts.
    pub(crate) const fn mount_allocator_mut(&mut self) -> &mut ViewMountAllocator {
        &mut self.allocator
    }

    /// Validates externally retained Rust-backed mounts against authored mounts
    /// and the same persisted allocator cursor.
    pub(crate) fn validate_reserved_mounts(
        &self,
        reserved: impl IntoIterator<Item = ViewMountId>,
    ) -> Result<(), BundleViewRuntimeError> {
        let authored = self
            .mounts
            .values()
            .map(|mount| mount.state.mount())
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for mount in reserved {
            if authored.contains(&mount) || !seen.insert(mount) {
                return Err(BundleViewRuntimeError::DuplicateMount { mount });
            }
            if mount.get() >= self.allocator.next() {
                return Err(BundleViewRuntimeError::UnallocatedMount {
                    mount,
                    next: self.allocator.next(),
                });
            }
        }
        Ok(())
    }

    /// Style-cascade diagnostics produced for typed text targets at construction.
    #[must_use]
    pub const fn text_style_diagnostics(&self) -> &ViewRuntimeControlStyleDiagnostics {
        &self.text_style_diagnostics
    }

    pub fn advance_millis(&mut self, milliseconds: u64) -> Result<(), BundleViewRuntimeError> {
        self.logical_time = self.logical_time.try_advance_millis(milliseconds)?;
        Ok(())
    }

    #[must_use]
    pub fn live_mount_count(&self) -> usize {
        self.mounts.len()
    }

    pub(crate) fn has_program(&self) -> bool {
        self.program.is_some()
    }

    pub(crate) fn definition_ids(&self) -> BTreeSet<String> {
        self.definitions.keys().cloned().collect()
    }

    #[must_use]
    pub fn snapshot(&self) -> BundleViewRuntimeSnapshot {
        BundleViewRuntimeSnapshot {
            program_id: self
                .program
                .as_ref()
                .map(|program| program.program_id.clone()),
            logical_time: self.logical_time,
            next_mount_id: self.allocator.next(),
            root_bindings: self
                .root_bindings
                .iter()
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            mounts: self
                .mounts
                .iter()
                .map(|(key, mount)| BundleViewMountRuntimeSnapshot {
                    handle: key.handle.clone(),
                    path: key.path.clone(),
                    definition: mount.definition.clone(),
                    activation_logical_time: mount.activation_logical_time,
                    deterministic_seed: mount.deterministic_seed,
                    state: mount.state.snapshot(),
                    initialized_parameters: mount.initialized_parameters.iter().copied().collect(),
                    initialized_state: mount.initialized_state.iter().copied().collect(),
                    runtime_parameters: mount
                        .runtime_parameters
                        .iter()
                        .map(|(name, value)| RuntimeBinding {
                            name: name.clone(),
                            value: value.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Restores an exact mount table atomically after validating every identity and slot.
    pub fn restore(
        &mut self,
        snapshot: &BundleViewRuntimeSnapshot,
    ) -> Result<(), BundleViewRuntimeError> {
        let expected_program = self
            .program
            .as_ref()
            .map(|program| program.program_id.clone());
        if snapshot.program_id != expected_program {
            return Err(BundleViewRuntimeError::ProgramMismatch {
                saved: snapshot.program_id.clone(),
                expected: expected_program,
            });
        }

        let mut root_bindings = BTreeMap::new();
        for binding in &snapshot.root_bindings {
            if root_bindings
                .insert(binding.name.clone(), binding.value.clone())
                .is_some()
            {
                return Err(BundleViewRuntimeError::DuplicateRootBinding {
                    binding: binding.name.clone(),
                });
            }
        }

        let mut mounts = BTreeMap::new();
        let mut mount_ids = BTreeSet::new();
        for saved in &snapshot.mounts {
            saved.path.validate()?;
            if saved.activation_logical_time.seconds().seconds()
                > snapshot.logical_time.seconds().seconds()
            {
                return Err(BundleViewRuntimeError::ActivationAfterRuntime);
            }
            let definition_index = self.definition_index(&saved.definition)?;
            let definition = self.definition(definition_index);
            let program_id = definition_program_id(definition_index)?;
            let state = ViewMountState::from_snapshot(
                &saved.state,
                program_id,
                definition.state_schema_hash,
                &self.inventory,
            )?;
            if !mount_ids.insert(state.mount()) {
                return Err(BundleViewRuntimeError::DuplicateMount {
                    mount: state.mount(),
                });
            }
            let initialized_parameters = validated_initialized_slots(
                "parameter",
                &saved.initialized_parameters,
                self.inventory.parameter_types(),
            )?;
            let initialized_state = validated_initialized_slots(
                "state",
                &saved.initialized_state,
                self.inventory.state_types(),
            )?;
            let mut runtime_parameters = BTreeMap::new();
            for parameter in &saved.runtime_parameters {
                if runtime_parameters
                    .insert(parameter.name.clone(), parameter.value.clone())
                    .is_some()
                {
                    return Err(BundleViewRuntimeError::DuplicateRuntimeParameter {
                        parameter: parameter.name.clone(),
                    });
                }
            }
            let key = ViewOccurrenceKey {
                handle: saved.handle.clone(),
                path: saved.path.clone(),
            };
            if mounts
                .insert(
                    key.clone(),
                    MountedView {
                        definition: saved.definition.clone(),
                        activation_logical_time: saved.activation_logical_time,
                        deterministic_seed: saved.deterministic_seed,
                        state,
                        initialized_parameters,
                        initialized_state,
                        runtime_parameters,
                    },
                )
                .is_some()
            {
                return Err(BundleViewRuntimeError::DuplicateOccurrence {
                    handle: key.handle,
                    path: key.path,
                });
            }
        }

        let greatest_live = mounts.values().map(|mount| mount.state.mount()).max();
        let mut allocator = ViewMountAllocator::default();
        allocator.restore_cursor(snapshot.next_mount_id, greatest_live)?;
        self.logical_time = snapshot.logical_time;
        self.allocator = allocator;
        self.root_bindings = root_bindings;
        self.mounts = mounts;
        Ok(())
    }

    pub(crate) fn validate_frame(
        &self,
        frame: &BundleViewFrame,
    ) -> Result<(), BundleViewRuntimeError> {
        let mut occurrences = BTreeSet::new();
        for output in &frame.mounts {
            let key = ViewOccurrenceKey {
                handle: output.handle.clone(),
                path: output.path.clone(),
            };
            if !occurrences.insert(key.clone()) {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "occurrence `{}` at path {:?} appears more than once",
                        output.handle, output.path
                    ),
                });
            }
            let Some(mounted) = self.mounts.get(&key) else {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "occurrence `{}` at path {:?} is not retained",
                        output.handle, output.path
                    ),
                });
            };
            if mounted.definition != output.view || mounted.state.mount() != output.mount {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "occurrence `{}` at path {:?} records view `{}`/mount {:?}, expected `{}`/{:?}",
                        output.handle,
                        output.path,
                        output.view,
                        output.mount,
                        mounted.definition,
                        mounted.state.mount()
                    ),
                });
            }
        }
        Ok(())
    }

    fn definition_index(&self, definition: &str) -> Result<usize, BundleViewRuntimeError> {
        self.definitions.get(definition).copied().ok_or_else(|| {
            BundleViewRuntimeError::UnknownDefinition {
                definition: definition.to_owned(),
            }
        })
    }

    fn definition(&self, index: usize) -> &ViewDefinitionResource {
        &self
            .program
            .as_ref()
            .expect("a definition index requires a View program")
            .definitions[index]
    }
}

fn validate_definition_span(
    definition: &ViewDefinitionResource,
    instruction_count: usize,
) -> Result<(), BundleViewRuntimeError> {
    let start = usize::try_from(definition.body.start_instruction).ok();
    let end = usize::try_from(definition.body.end_instruction).ok();
    if !matches!((start, end), (Some(start), Some(end)) if start <= end && end <= instruction_count)
    {
        return Err(BundleViewRuntimeError::InvalidDefinitionSpan {
            definition: definition.public_id.clone(),
        });
    }
    Ok(())
}

fn definition_program_id(index: usize) -> Result<ViewProgramId, BundleViewRuntimeError> {
    u32::try_from(index).map(ViewProgramId).map_err(|_| {
        BundleViewRuntimeError::InvalidDefinitionSpan {
            definition: format!("definition[{index}]"),
        }
    })
}

fn validated_initialized_slots(
    kind: &'static str,
    slots: &[u16],
    types: &[FxRuntimeType],
) -> Result<BTreeSet<u16>, BundleViewRuntimeError> {
    let mut validated = BTreeSet::new();
    for slot in slots {
        if usize::from(*slot) >= types.len() || !validated.insert(*slot) {
            return Err(BundleViewRuntimeError::InvalidInitializedSlot { kind, slot: *slot });
        }
    }
    Ok(validated)
}

fn deterministic_mount_seed(
    handle: &PresentationHandleId,
    path: &BundleViewInstancePath,
    definition: &str,
) -> u64 {
    let mut transcript = b"arcweft.view-mount.v1".to_vec();
    append_seed_part(&mut transcript, handle.as_str().as_bytes());
    append_seed_part(&mut transcript, definition.as_bytes());
    for segment in path.segments() {
        match segment {
            BundleViewInstancePathSegment::Call {
                instruction,
                authored_key,
            } => {
                transcript.push(0);
                transcript.extend_from_slice(&instruction.to_le_bytes());
                match authored_key {
                    Some(key) => {
                        transcript.push(1);
                        transcript.extend_from_slice(&key.to_le_bytes());
                    }
                    None => transcript.push(0),
                }
            }
            BundleViewInstancePathSegment::Repeat { instruction, key } => {
                transcript.push(1);
                transcript.extend_from_slice(&instruction.to_le_bytes());
                transcript.extend_from_slice(&key.to_le_bytes());
            }
        }
    }
    let bytes = BundleDigest::of(&transcript).as_bytes();
    u64::from_le_bytes(
        bytes[..8]
            .try_into()
            .expect("digest prefix has eight bytes"),
    )
}

fn append_seed_part(transcript: &mut Vec<u8>, bytes: &[u8]) {
    transcript.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    transcript.extend_from_slice(bytes);
}
