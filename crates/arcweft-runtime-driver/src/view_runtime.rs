//! Deterministic execution state for bundle-authored View programs.
//!
//! A View definition may be mounted by more than one presentation handle and
//! may recursively mount child definitions. This module owns those occurrence
//! identities, persistent typed value slots, activation-relative logical time,
//! exact save state, and renderer-neutral frame output.

mod axis_seed;
mod catalog;
mod evaluator;
mod owner;
#[cfg(test)]
mod owner_tests;
mod part;
mod replacement;
mod style_scope;
mod value;

use crate::dialogue::{BundlePresentationInput, DialogueViewInput, DialogueViewState};
use crate::presentation_handles::{PresentationHandleId, PresentationHandleRecord};
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::resource_codec::view::{
    DialogueViewContractError, ValidatedViewProduct, ViewObserveClassification,
    ViewProductValidationLimits, ViewTextSelectionPolicy, ViewTextSourceKind,
};
use arcweft_bundle::resource_codec::{
    ViewDefinitionResource, ViewRuntimeControlVisualStyle, ViewTextBlockBounds, ViewTextResource,
};
use arcweft_core::{
    awbc::schema::AwbcProgram,
    value::{
        AwbcRuntimeValueSnapshot, RuntimeBinding, RuntimeDialogueActionValue,
        RuntimeDialogueAdvanceAction, RuntimeValue,
    },
};
use arcweft_id::PublicId;
use arcweft_presentation::fx::{
    FiniteF32Error, FxGraphChildPath, FxId, FxInstanceId, FxLogicalTime, FxRuntimeType,
    FxRuntimeValue,
};
use arcweft_presentation::input::InteractionTarget;
use arcweft_text_model::{LineDisplayFrame, RichTextDocument};
use arcweft_view::{
    EventKind, ViewHandlerInvocation, ViewHandlerProgramId, ViewHandlerRouteId, ViewId,
    ViewMountAllocationError, ViewMountAllocator, ViewMountId, ViewMountSnapshot, ViewMountState,
    ViewPartName, ViewProgramId, ViewRegistry, ViewRegistryError, ViewRegistryId, ViewSchemaId,
    ViewStyleProgram, ViewValueEvaluationError, ViewValueInventoryError, ViewValueProgramInventory,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use thiserror::Error;

pub use axis_seed::{
    BundleViewAxisSeedError, BundleViewAxisSeedRegistrySnapshot, BundleViewAxisSeedUpdate,
    BundleViewAxisSeedUpdateOutcome, BundleViewMountedAxisSeedSnapshot,
    BundleViewPendingAxisSeedSnapshot,
};
pub use catalog::{ViewProgramCatalog, ViewProgramCatalogError};
pub use owner::{AcceptedViewProgramGeneration, SavedViewOwner, ViewOwnerEvidence, ViewSaveError};
pub use replacement::{
    PreparedViewProgramReplacement, ViewMountReconcileError, ViewProgramInvalidation,
    ViewProgramReplacementError, ViewProgramReplacementOutcome,
};

pub use style_scope::{BundleViewStyleNode, BundleViewStyleNodeId, BundleViewStyleNodeKind};
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
    MissingDialogueInput,
    InvalidDialogueViewOwner,
    InvalidHandler,
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
            Self::MissingDialogueInput => "VIEW018_MISSING_DIALOGUE_INPUT",
            Self::InvalidDialogueViewOwner => "VIEW019_INVALID_DIALOGUE_VIEW_OWNER",
            Self::InvalidHandler => "VIEW020_INVALID_HANDLER",
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

impl BundleViewDiagnostic {
    fn invalid_dialogue_view_owner(error: &BundleViewRuntimeError) -> Self {
        Self {
            code: BundleViewDiagnosticCode::InvalidDialogueViewOwner,
            handle: None,
            mount: None,
            view: error
                .dialogue_view_definition()
                .map(|definition| definition.as_str().to_owned()),
            instruction: None,
            message: error.to_string(),
        }
    }
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
    /// Character display name retaining the same resolved frame provenance as content.
    CharacterDisplayName {
        frame: Box<LineDisplayFrame>,
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
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub style: ViewRuntimeControlVisualStyle,
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
    pub view: ViewId,
    pub path: BundleViewInstancePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_axis_seed: Option<arcweft_view::ViewInheritedBoxAxes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<DialogueViewState>,
    pub active_targets: Vec<String>,
    pub active_images: Vec<String>,
    pub paint: Vec<BundleViewPaintItem>,
    pub text: Vec<BundleViewTextOutput>,
    pub fx: Vec<BundleViewFxApplication>,
    /// Typed event routes published only after all mount handlers seal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<BundleViewEventBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_nodes: Vec<BundleViewStyleNode>,
}

/// Exact retained node/event route for one sealed mount-time handler token.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BundleViewEventBinding {
    route: ViewHandlerRouteId,
    target: InteractionTarget,
    mount: ViewMountId,
    path: BundleViewInstancePath,
    instruction: u32,
    event: EventKind,
    program: ViewHandlerProgramId,
}

impl BundleViewEventBinding {
    #[must_use]
    pub const fn route(&self) -> ViewHandlerRouteId {
        self.route
    }

    #[must_use]
    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    #[must_use]
    pub const fn mount(&self) -> ViewMountId {
        self.mount
    }

    #[must_use]
    pub const fn path(&self) -> &BundleViewInstancePath {
        &self.path
    }

    #[must_use]
    pub const fn instruction(&self) -> u32 {
        self.instruction
    }

    #[must_use]
    pub const fn event(&self) -> EventKind {
        self.event
    }

    #[must_use]
    pub const fn program(&self) -> ViewHandlerProgramId {
        self.program
    }

    /// Projects this accepted route into the retained fragment event algebra.
    #[must_use]
    pub const fn retained_binding(&self) -> arcweft_view::EventBinding {
        arcweft_view::EventBinding::new(self.event, self.route)
    }
}

/// Fail-closed event-token routing error.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BundleViewEventDispatchError {
    #[error("View event binding is not active in the latest successful frame")]
    UnknownBinding,
    #[error("routed View handler invocation does not match the active binding")]
    InvocationMismatch,
    #[error("sealed DialogueAction token is invalid: {message}")]
    InvalidDialogueAction { message: String },
}

/// Public owner and part identity projected from an accepted exported-part boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewExportedPartEvidence {
    /// Stable public owner of the exported part.
    pub owner: ViewOwnerEvidence,
    /// Public part capability name; the owner-local implementation name is omitted.
    pub part: ViewPartName,
}

impl BundleViewMountOutput {
    /// Qualifies one authored resource identity for this concrete mount occurrence.
    #[must_use]
    pub fn scoped_id(&self, authored: &str) -> String {
        mount_scoped_id(self.mount, authored)
    }

    /// Projects exported-part evidence without exposing local or dense identities.
    pub fn exported_part_evidence(
        &self,
    ) -> impl Iterator<Item = BundleViewExportedPartEvidence> + '_ {
        self.style_nodes.iter().filter_map(|node| {
            node.exported_part
                .as_ref()
                .map(|part| BundleViewExportedPartEvidence {
                    owner: ViewOwnerEvidence::Public {
                        view: self.view.clone(),
                    },
                    part: part.clone(),
                })
        })
    }
}

fn mount_scoped_id(mount: ViewMountId, authored: &str) -> String {
    format!("view_mount_{}.{}", mount.get(), authored)
}

fn mount_scoped_interaction_target(
    mount: ViewMountId,
    authored: &str,
) -> Result<InteractionTarget, arcweft_id::IdError> {
    PublicId::try_new_engine_owned(mount_scoped_id(mount, authored)).map(InteractionTarget::new)
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
    pub program_id: Option<ViewProgramId>,
    pub logical_time: FxLogicalTime,
    pub next_mount_id: u64,
    pub view_root_bindings: Vec<RuntimeBinding>,
    pub mounts: Vec<BundleViewMountRuntimeSnapshot>,
    pub axis_seeds: BundleViewAxisSeedRegistrySnapshot,
}

/// Exact persisted state for one root or nested mounted View occurrence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleViewMountRuntimeSnapshot {
    pub handle: PresentationHandleId,
    pub path: BundleViewInstancePath,
    pub owner: SavedViewOwner,
    pub activation_logical_time: FxLogicalTime,
    pub deterministic_seed: u64,
    pub state: ViewMountSnapshot,
    pub initialized_parameters: Vec<u16>,
    pub initialized_state: Vec<u16>,
    pub runtime_parameters: Vec<RuntimeBinding>,
    pub next_handler_seal_revision: u64,
}

/// Fatal construction or snapshot restoration failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum BundleViewRuntimeError {
    #[error(transparent)]
    AxisSeed(BundleViewAxisSeedError),
    #[error(transparent)]
    Program(#[from] arcweft_bundle::resource_codec::SectionCodecError),
    #[error(transparent)]
    Catalog(#[from] ViewProgramCatalogError),
    #[error(transparent)]
    ProductValidation(#[from] arcweft_bundle::resource_codec::ViewProductValidationError),
    #[error(transparent)]
    Registry(#[from] ViewRegistryError),
    #[error(transparent)]
    Save(#[from] ViewSaveError),
    #[error(transparent)]
    DialogueContract(#[from] DialogueViewContractError),
    #[error(transparent)]
    Inventory(#[from] ViewValueInventoryError),
    #[error(transparent)]
    MountAllocation(#[from] ViewMountAllocationError),
    #[error(transparent)]
    MountState(#[from] ViewValueEvaluationError),
    #[error(transparent)]
    LogicalTime(#[from] FiniteF32Error),
    #[error("View program repeats definition `{definition}`")]
    DuplicateDefinition { definition: ViewId },
    #[error("View definition `{definition}` has an invalid instruction span")]
    InvalidDefinitionSpan { definition: ViewId },
    #[error("View snapshot belongs to program {saved:?}, expected {expected:?}")]
    ProgramMismatch {
        saved: Option<ViewProgramId>,
        expected: Option<ViewProgramId>,
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
    UnknownDefinition { definition: ViewId },
    #[error("dialogue presentation selects unknown View definition `{definition}`")]
    UnknownDialogueViewDefinition { definition: ViewId },
    #[error("dialogue presentation selects View `{definition}` without a dialogue input parameter")]
    DialogueViewDefinitionMissingRole { definition: ViewId },
    #[error("dialogue presentation selects unauthorized View definition `{definition}`")]
    UnauthorizedDialogueViewDefinition { definition: ViewId },
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
    #[error("View snapshot has an invalid handler seal revision cursor")]
    InvalidHandlerSealRevision,
}

impl BundleViewRuntimeError {
    fn dialogue_view_definition(&self) -> Option<&ViewId> {
        match self {
            Self::UnknownDialogueViewDefinition { definition }
            | Self::DialogueViewDefinitionMissingRole { definition }
            | Self::UnauthorizedDialogueViewDefinition { definition } => Some(definition),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ViewOccurrenceKey {
    handle: PresentationHandleId,
    path: BundleViewInstancePath,
}

#[derive(Clone, Debug, PartialEq)]
struct MountedView {
    owner: owner::ResolvedMountedViewOwner,
    activation_logical_time: FxLogicalTime,
    deterministic_seed: u64,
    state: ViewMountState,
    initialized_parameters: BTreeSet<u16>,
    initialized_state: BTreeSet<u16>,
    runtime_parameters: BTreeMap<String, RuntimeValue>,
    handler_seals: BTreeMap<MountedViewHandlerKey, MountedViewHandlerSeal>,
    next_handler_seal_revision: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MountedViewHandlerKey {
    path: BundleViewInstancePath,
    instruction: u32,
    event: EventKind,
    program: ViewHandlerProgramId,
}

#[derive(Clone, Debug, PartialEq)]
struct MountedViewHandlerSeal {
    captures: Box<[AwbcRuntimeValueSnapshot]>,
    token: RuntimeDialogueActionToken,
    revision: u64,
}

impl MountedView {
    fn view(&self) -> &ViewId {
        self.owner
            .view()
            .expect("the bundle evaluator stores only public Arcweft View owners")
    }
}

/// Sans I/O evaluator and persistent mount table for one active View program.
#[derive(Clone, Debug)]
pub struct BundleViewRuntime {
    product: ValidatedViewProduct,
    catalog: Option<ViewProgramCatalog>,
    registry: ViewRegistry,
    generation: AcceptedViewProgramGeneration,
    frame_revision: u64,
    last_invalidation: Option<ViewProgramInvalidation>,
    style_program: Option<ViewStyleProgram>,
    text: Option<ViewTextResource>,
    inventory: ViewValueProgramInventory,
    logical_time: FxLogicalTime,
    allocator: ViewMountAllocator,
    view_root_bindings: BTreeMap<String, RuntimeValue>,
    mounts: BTreeMap<ViewOccurrenceKey, MountedView>,
    axis_seeds: axis_seed::BundleViewAxisSeedRegistry,
    required_dialogue_views: BTreeSet<ViewId>,
    handler_runtime: ViewHandlerRuntimeAuthority,
    event_tokens: BTreeMap<ViewHandlerRouteId, PublishedViewEventToken>,
}

#[derive(Clone, Debug, PartialEq)]
struct PublishedViewEventToken {
    event: EventKind,
    target: InteractionTarget,
    token: RuntimeDialogueActionToken,
}

#[derive(Clone, Debug)]
enum ViewHandlerRuntimeAuthority {
    HandlerFree,
    Awbc(Arc<AwbcProgram>),
}

impl Default for BundleViewRuntime {
    fn default() -> Self {
        let product =
            ValidatedViewProduct::try_new(None, None, None, ViewProductValidationLimits::default())
                .expect("an empty View product is valid");
        Self::try_new(product, None).expect("an empty View runtime is valid")
    }
}

impl BundleViewInstancePath {
    #[must_use]
    pub fn segments(&self) -> &[BundleViewInstancePathSegment] {
        &self.0
    }

    /// Total platform-independent physical Style path encoding.
    #[must_use]
    pub fn style_path_words(&self) -> Vec<u64> {
        let mut words = Vec::with_capacity(self.0.len() * 4);
        for segment in &self.0 {
            segment.append_style_path_words(&mut words);
        }
        words
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

impl BundleViewInstancePathSegment {
    /// Appends this closed path segment's four-word little-endian Style identity.
    pub fn append_style_path_words(&self, output: &mut Vec<u64>) {
        match self {
            Self::Call {
                instruction,
                authored_key,
            } => output.extend([
                0,
                u64::from(*instruction),
                u64::from(authored_key.is_some()),
                authored_key.unwrap_or(0),
            ]),
            Self::Repeat { instruction, key } => output.extend([
                1,
                u64::from(*instruction),
                u64::from(u32::from_le_bytes(key.to_le_bytes())),
                0,
            ]),
        }
    }
}

impl BundleViewRuntime {
    /// Builds an evaluator only from a complete validated View product.
    pub fn try_new(
        product: ValidatedViewProduct,
        text: Option<ViewTextResource>,
    ) -> Result<Self, BundleViewRuntimeError> {
        Self::try_new_with_registry(product, text, ViewRegistry::default())
    }

    /// Builds a handler-capable evaluator from the exact bundle AWBC authority.
    pub fn try_new_with_awbc(
        product: ValidatedViewProduct,
        text: Option<ViewTextResource>,
        awbc: Arc<AwbcProgram>,
    ) -> Result<Self, BundleViewRuntimeError> {
        Self::try_new_with_registry_and_awbc(product, text, ViewRegistry::default(), awbc)
    }

    /// Builds an evaluator while preserving already registered host Views.
    ///
    /// The supplied registry is consumed as a candidate. Arcweft definitions
    /// are inserted only into that candidate, so a public-owner collision
    /// fails without mutating any previously published registry.
    pub fn try_new_with_registry(
        product: ValidatedViewProduct,
        text: Option<ViewTextResource>,
        registry: ViewRegistry,
    ) -> Result<Self, BundleViewRuntimeError> {
        match product.program() {
            Some(program) => program
                .resource()
                .validate_dialogue_contract(text.as_ref())?,
            None => {
                if let Some(source) = text.as_ref().and_then(|text| {
                    text.sources
                        .iter()
                        .find(|source| matches!(source.kind, ViewTextSourceKind::Dialogue { .. }))
                }) {
                    return Err(DialogueViewContractError::MissingProgram {
                        text_source: source.public_id.clone(),
                    }
                    .into());
                }
            }
        }
        let catalog = ViewProgramCatalog::try_from_validated(&product)?;
        Self::finish_construction(
            product,
            text,
            registry,
            catalog,
            ViewHandlerRuntimeAuthority::HandlerFree,
        )
    }

    pub fn try_new_with_registry_and_awbc(
        product: ValidatedViewProduct,
        text: Option<ViewTextResource>,
        registry: ViewRegistry,
        awbc: Arc<AwbcProgram>,
    ) -> Result<Self, BundleViewRuntimeError> {
        match product.program() {
            Some(program) => program
                .resource()
                .validate_dialogue_contract(text.as_ref())?,
            None => {
                if let Some(source) = text.as_ref().and_then(|text| {
                    text.sources
                        .iter()
                        .find(|source| matches!(source.kind, ViewTextSourceKind::Dialogue { .. }))
                }) {
                    return Err(DialogueViewContractError::MissingProgram {
                        text_source: source.public_id.clone(),
                    }
                    .into());
                }
            }
        }
        let catalog = ViewProgramCatalog::try_from_validated_with_awbc(&product, &awbc)?;
        Self::finish_construction(
            product,
            text,
            registry,
            catalog,
            ViewHandlerRuntimeAuthority::Awbc(awbc),
        )
    }

    fn finish_construction(
        product: ValidatedViewProduct,
        text: Option<ViewTextResource>,
        mut registry: ViewRegistry,
        catalog: Option<ViewProgramCatalog>,
        handler_runtime: ViewHandlerRuntimeAuthority,
    ) -> Result<Self, BundleViewRuntimeError> {
        if let Some(catalog) = &catalog {
            for (view, definition) in catalog.definitions() {
                registry.register_arcweft(
                    view.clone(),
                    ViewSchemaId(definition.state_schema_hash()),
                    catalog.program_id().clone(),
                    catalog.revision(),
                )?;
            }
        }
        let inventory = ViewValueProgramInventory::from_programs(
            catalog.as_ref().map_or_else(Vec::new, |catalog| {
                catalog.resource().value_programs.clone()
            }),
        )?;
        let style_program = product.style().map(|style| style.program().clone());
        Ok(Self {
            product,
            catalog,
            registry,
            generation: AcceptedViewProgramGeneration::INITIAL,
            frame_revision: 0,
            last_invalidation: None,
            style_program,
            text,
            inventory,
            logical_time: FxLogicalTime::zero(),
            allocator: ViewMountAllocator::default(),
            view_root_bindings: BTreeMap::new(),
            mounts: BTreeMap::new(),
            axis_seeds: axis_seed::BundleViewAxisSeedRegistry::default(),
            required_dialogue_views: BTreeSet::new(),
            handler_runtime,
            event_tokens: BTreeMap::new(),
        })
    }

    /// Returns the immutable registry accepted with the current View product.
    #[must_use]
    pub const fn registry(&self) -> &ViewRegistry {
        &self.registry
    }

    /// Dispatches one latest-frame event from its sealed DialogueAction token.
    /// The associated pure helper is never re-entered at this boundary.
    pub fn dispatch_invocation(
        &self,
        invocation: &ViewHandlerInvocation,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        let published = self
            .event_tokens
            .get(&invocation.route())
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        if published.event != invocation.event() || &published.target != invocation.target() {
            return Err(BundleViewEventDispatchError::InvocationMismatch);
        }
        published.token.presentation_input()
    }

    /// Projects a live registry entry into stable public owner evidence.
    #[must_use]
    pub fn registry_owner_evidence(&self, id: ViewRegistryId) -> Option<ViewOwnerEvidence> {
        owner::ResolvedMountedViewOwner::resolve_registry(
            id,
            &self.registry,
            self.catalog.as_ref(),
            self.generation,
        )
        .ok()
        .map(|owner| owner.evidence())
    }

    /// Returns the immutable accepted Arcweft catalog, when the product has one.
    #[must_use]
    pub const fn catalog(&self) -> Option<&ViewProgramCatalog> {
        self.catalog.as_ref()
    }

    /// Returns the complete validated product owned by this runtime generation.
    #[must_use]
    pub const fn product(&self) -> &ValidatedViewProduct {
        &self.product
    }

    pub(crate) fn validate_dialogue_inputs(
        &mut self,
        dialogue: &[DialogueViewInput<'_>],
    ) -> Result<(), BundleViewRuntimeError> {
        self.validate_dialogue_input_owners(dialogue)?;
        self.required_dialogue_views
            .extend(dialogue.iter().map(|input| input.view.clone()));
        Ok(())
    }

    pub(crate) fn transient_dialogue_view_owners(&self) -> Vec<ViewId> {
        self.required_dialogue_views.iter().cloned().collect()
    }

    fn validate_dialogue_input_owners(
        &self,
        dialogue: &[DialogueViewInput<'_>],
    ) -> Result<(), BundleViewRuntimeError> {
        let catalog = self.catalog.as_ref();
        for input in dialogue {
            let Some(catalog) = catalog else {
                return Err(BundleViewRuntimeError::UnknownDialogueViewDefinition {
                    definition: input.view.clone(),
                });
            };
            if catalog.definition_index(input.view).is_none() {
                return Err(BundleViewRuntimeError::UnknownDialogueViewDefinition {
                    definition: input.view.clone(),
                });
            }
            if !catalog.accepts_dialogue_input(input.view) {
                return Err(BundleViewRuntimeError::DialogueViewDefinitionMissingRole {
                    definition: input.view.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_authorized_dialogue_inputs(
        &self,
        dialogue: &[DialogueViewInput<'_>],
    ) -> Result<(), BundleViewRuntimeError> {
        self.validate_dialogue_input_owners(dialogue)?;
        for input in dialogue {
            if !self.required_dialogue_views.contains(input.view) {
                return Err(BundleViewRuntimeError::UnauthorizedDialogueViewDefinition {
                    definition: input.view.clone(),
                });
            }
        }
        Ok(())
    }

    fn accepted_program_id(&self) -> Option<&ViewProgramId> {
        self.catalog.as_ref().map(ViewProgramCatalog::program_id)
    }

    #[must_use]
    pub const fn logical_time(&self) -> FxLogicalTime {
        self.logical_time
    }

    /// Validates store, retained mount, and serialized frame identity as one
    /// atomic dialogue View save-point contract.
    pub(crate) fn validate_dialogue_snapshot(
        &self,
        frame: &BundleViewFrame,
        dialogue: &[DialogueViewInput<'_>],
        presentation_handles: &[PresentationHandleRecord],
    ) -> Result<(), BundleViewRuntimeError> {
        self.validate_authorized_dialogue_inputs(dialogue)?;
        let mut expected = BTreeMap::new();
        for input in dialogue {
            if expected
                .insert(input.handle.clone(), (input.view, input.state))
                .is_some()
            {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{}` appears more than once in the presentation store",
                        input.handle
                    ),
                });
            }
        }

        let output_occurrences = self.validate_dialogue_frame_outputs(frame, &expected)?;
        self.validate_snapshot_mount_owners(&expected, presentation_handles)?;
        let retained_occurrences = self
            .mounts
            .keys()
            .filter(|key| expected.contains_key(&key.handle))
            .cloned()
            .collect::<BTreeSet<_>>();
        if let Some(missing) = retained_occurrences.difference(&output_occurrences).next() {
            return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                message: format!(
                    "dialogue occurrence `{}` at path {:?} has no serialized View output",
                    missing.handle, missing.path
                ),
            });
        }

        for (handle, (view, _)) in &expected {
            let key = ViewOccurrenceKey {
                handle: handle.clone(),
                path: BundleViewInstancePath::default(),
            };
            let mounted = self.mounts.get(&key).ok_or_else(|| {
                BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{handle}` has no retained root View mount"
                    ),
                }
            })?;
            if mounted.view() != *view {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{}` retains View `{}`, expected `{}`",
                        handle,
                        mounted.view(),
                        view
                    ),
                });
            }
            if !output_occurrences.contains(&key) {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{handle}` has no serialized root View output"
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_snapshot_mount_owners(
        &self,
        dialogue: &BTreeMap<PresentationHandleId, (&ViewId, DialogueViewState)>,
        presentation_handles: &[PresentationHandleRecord],
    ) -> Result<(), BundleViewRuntimeError> {
        let ordinary = presentation_handles
            .iter()
            .filter(|handle| !handle.is_terminal())
            .filter(|handle| {
                ViewId::parse_public(handle.resource_id.clone())
                    .ok()
                    .and_then(|view| self.catalog.as_ref()?.definition_index(&view))
                    .is_some()
            })
            .map(|handle| handle.id.clone())
            .collect::<BTreeSet<_>>();
        if let Some(orphan) = self
            .mounts
            .keys()
            .find(|key| !dialogue.contains_key(&key.handle) && !ordinary.contains(&key.handle))
        {
            return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                message: format!(
                    "retained View occurrence `{}` at path {:?} has no live presentation owner",
                    orphan.handle, orphan.path
                ),
            });
        }
        Ok(())
    }

    fn validate_dialogue_frame_outputs(
        &self,
        frame: &BundleViewFrame,
        expected: &BTreeMap<PresentationHandleId, (&ViewId, DialogueViewState)>,
    ) -> Result<BTreeSet<ViewOccurrenceKey>, BundleViewRuntimeError> {
        let mut occurrences = BTreeSet::new();
        for output in &frame.mounts {
            let expectation = expected.get(&output.handle);
            if output.dialogue.is_some() && expectation.is_none() {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "serialized View output `{}` at path {:?} has dialogue state but no presentation-store occurrence",
                        output.handle, output.path
                    ),
                });
            }
            let Some((_, state)) = expectation else {
                continue;
            };
            if output.dialogue != Some(*state) {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{}` at path {:?} serialized state does not match its presentation store",
                        output.handle, output.path
                    ),
                });
            }
            let key = ViewOccurrenceKey {
                handle: output.handle.clone(),
                path: output.path.clone(),
            };
            if !occurrences.insert(key.clone()) {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{}` at path {:?} appears more than once in the serialized frame",
                        output.handle, output.path
                    ),
                });
            }
            let mounted = self.mounts.get(&key).ok_or_else(|| {
                BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "serialized dialogue View output `{}` at path {:?} has no retained mount",
                        output.handle, output.path
                    ),
                }
            })?;
            if mounted.view() != &output.view || mounted.state.mount() != output.mount {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "dialogue occurrence `{}` at path {:?} records view `{}`/mount {:?}, expected `{}`/{:?}",
                        output.handle,
                        output.path,
                        output.view,
                        output.mount,
                        mounted.view(),
                        mounted.state.mount()
                    ),
                });
            }
        }
        Ok(occurrences)
    }

    /// Canonical native Style program retained for live player resolution.
    #[must_use]
    pub const fn style_program(&self) -> Option<&ViewStyleProgram> {
        self.style_program.as_ref()
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
        self.catalog.is_some()
    }

    pub(crate) fn definition_ids(&self) -> BTreeSet<ViewId> {
        self.catalog
            .as_ref()
            .map(|catalog| catalog.view_ids().cloned().collect())
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> Result<BundleViewRuntimeSnapshot, ViewSaveError> {
        let mounts = self
            .mounts
            .iter()
            .map(|(key, mount)| {
                Ok(BundleViewMountRuntimeSnapshot {
                    handle: key.handle.clone(),
                    path: key.path.clone(),
                    owner: mount.owner.saved(&self.registry)?,
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
                    next_handler_seal_revision: mount.next_handler_seal_revision,
                })
            })
            .collect::<Result<Vec<_>, ViewSaveError>>()?;
        Ok(BundleViewRuntimeSnapshot {
            program_id: self
                .catalog
                .as_ref()
                .map(|catalog| catalog.program_id().clone()),
            logical_time: self.logical_time,
            next_mount_id: self.allocator.next(),
            view_root_bindings: self
                .view_root_bindings
                .iter()
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            mounts,
            axis_seeds: self.axis_seeds.snapshot(),
        })
    }

    /// Restores an exact mount table atomically after validating every identity and slot.
    #[expect(
        clippy::too_many_lines,
        reason = "snapshot restore preflights the complete mount table, allocator, bindings, and axis registry before one atomic commit"
    )]
    pub fn restore(
        &mut self,
        snapshot: &BundleViewRuntimeSnapshot,
        reconciled_root_handles: &[PresentationHandleRecord],
    ) -> Result<(), BundleViewRuntimeError> {
        let expected_program = self
            .catalog
            .as_ref()
            .map(|catalog| catalog.program_id().clone());
        if snapshot.program_id != expected_program {
            return Err(BundleViewRuntimeError::ProgramMismatch {
                saved: snapshot.program_id.clone(),
                expected: expected_program,
            });
        }

        let mut view_root_bindings = BTreeMap::new();
        for binding in &snapshot.view_root_bindings {
            if view_root_bindings
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
            if saved.next_handler_seal_revision == 0 {
                return Err(BundleViewRuntimeError::InvalidHandlerSealRevision);
            }
            saved.path.validate()?;
            if saved.activation_logical_time.seconds().seconds()
                > snapshot.logical_time.seconds().seconds()
            {
                return Err(BundleViewRuntimeError::ActivationAfterRuntime);
            }
            let owner = owner::ResolvedMountedViewOwner::resolve_saved(
                &saved.owner,
                &self.registry,
                self.catalog.as_ref(),
                self.generation,
            )?;
            let definition_index = owner
                .definition()
                .ok_or(ViewSaveError::ImplementationKindMismatch)?;
            let saved_view = owner
                .view()
                .ok_or(ViewSaveError::ImplementationKindMismatch)?;
            let definition = self.definition(definition_index);
            let program_id = self.accepted_program_id().ok_or_else(|| {
                BundleViewRuntimeError::UnknownDefinition {
                    definition: saved_view.clone(),
                }
            })?;
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
                        owner,
                        activation_logical_time: saved.activation_logical_time,
                        deterministic_seed: saved.deterministic_seed,
                        state,
                        initialized_parameters,
                        initialized_state,
                        runtime_parameters,
                        handler_seals: BTreeMap::new(),
                        next_handler_seal_revision: saved.next_handler_seal_revision,
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
        let roots = mounts
            .iter()
            .filter(|(key, _)| key.path.segments().is_empty())
            .map(|(key, mount)| (mount.state.mount(), key.handle.clone()))
            .collect::<BTreeMap<_, _>>();
        let axis_seeds = axis_seed::BundleViewAxisSeedRegistry::restore(
            &snapshot.axis_seeds,
            &roots,
            reconciled_root_handles,
        )?;
        let mut allocator = ViewMountAllocator::default();
        allocator.restore_cursor(snapshot.next_mount_id, greatest_live)?;
        self.logical_time = snapshot.logical_time;
        self.allocator = allocator;
        self.view_root_bindings = view_root_bindings;
        self.mounts = mounts;
        self.axis_seeds = axis_seeds;
        self.event_tokens.clear();
        Ok(())
    }

    pub(crate) fn configure_next_axis_seed(
        &mut self,
        handle: PresentationHandleId,
        seed: arcweft_view::ViewBoxAxisHostSeed,
        handles: &[PresentationHandleRecord],
    ) -> Result<(), BundleViewAxisSeedError> {
        self.axis_seeds.configure_next(handle, seed, handles)
    }

    pub(crate) fn cancel_next_axis_seed(
        &mut self,
        handle: &PresentationHandleId,
    ) -> Option<arcweft_view::ViewBoxAxisHostSeed> {
        self.axis_seeds.cancel_next(handle)
    }

    pub(crate) fn update_axis_seed(
        &mut self,
        update: BundleViewAxisSeedUpdate,
    ) -> Result<BundleViewAxisSeedUpdateOutcome, BundleViewAxisSeedError> {
        if self.axis_seeds.mounted_seed(update.mount).is_none()
            && self.mounts.iter().any(|(key, mounted)| {
                mounted.state.mount() == update.mount && !key.path.segments().is_empty()
            })
        {
            return Err(BundleViewAxisSeedError::NestedMount {
                mount: update.mount,
            });
        }
        self.axis_seeds.update(update)
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
            if mounted.view() != &output.view || mounted.state.mount() != output.mount {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "occurrence `{}` at path {:?} records view `{}`/mount {:?}, expected `{}`/{:?}",
                        output.handle,
                        output.path,
                        output.view,
                        output.mount,
                        mounted.view(),
                        mounted.state.mount()
                    ),
                });
            }
            if output.path.segments().is_empty() {
                let expected = self.axis_seeds.mounted_seed(output.mount).ok_or_else(|| {
                    BundleViewRuntimeError::PresentationFrameMismatch {
                        message: format!(
                            "root occurrence `{}` at mount {:?} has no retained host axis seed",
                            output.handle, output.mount
                        ),
                    }
                })?;
                if output.host_axis_seed != Some(expected) {
                    return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                        message: format!(
                            "root occurrence `{}` at mount {:?} records host axis seed {:?}, expected {:?}",
                            output.handle, output.mount, output.host_axis_seed, expected
                        ),
                    });
                }
            } else if output.host_axis_seed.is_some() {
                return Err(BundleViewRuntimeError::PresentationFrameMismatch {
                    message: format!(
                        "nested occurrence `{}` at path {:?} unexpectedly records a host axis seed",
                        output.handle, output.path
                    ),
                });
            }
        }
        Ok(())
    }

    fn definition(&self, index: catalog::ViewDefinitionIndex) -> &ViewDefinitionResource {
        self.catalog
            .as_ref()
            .expect("a definition index requires a View program")
            .execution_definition(index)
    }
}

/// Runtime-driver owner of the closed DialogueAction payload and snapshot ABI.
#[derive(Clone, Debug, PartialEq)]
struct RuntimeDialogueActionToken {
    snapshot: AwbcRuntimeValueSnapshot,
}

impl RuntimeDialogueActionToken {
    fn from_target(target: Option<arcweft_view::DialogueAdvanceTarget>) -> Self {
        let value = match target {
            None => RuntimeDialogueActionValue::None,
            Some(target) => RuntimeDialogueActionValue::Advance(RuntimeDialogueAdvanceAction {
                dialogue: target.dialogue.get(),
                entry: target.entry.get(),
                instance: target.instance.get(),
                stage: target.stage.get(),
                revision: target.revision.get(),
            }),
        };
        Self::try_from_runtime_value(value.into_runtime_value())
            .expect("the DialogueAction owner produces its own closed payload schema")
    }

    fn try_from_runtime_value(value: RuntimeValue) -> Result<Self, BundleViewEventDispatchError> {
        Self::decode(&value)?;
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&value).map_err(|error| {
            BundleViewEventDispatchError::InvalidDialogueAction {
                message: error.to_string(),
            }
        })?;
        Ok(Self { snapshot })
    }

    fn runtime_value(&self) -> Result<RuntimeValue, BundleViewEventDispatchError> {
        self.snapshot.clone().into_runtime_value().map_err(|error| {
            BundleViewEventDispatchError::InvalidDialogueAction {
                message: error.to_string(),
            }
        })
    }

    fn presentation_input(
        &self,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        Self::decode(&self.runtime_value()?)
            .map(|target| target.map(BundlePresentationInput::advance_dialogue))
    }

    fn decode(
        value: &RuntimeValue,
    ) -> Result<Option<arcweft_view::DialogueAdvanceTarget>, BundleViewEventDispatchError> {
        match RuntimeDialogueActionValue::try_from_runtime_value(value).map_err(|error| {
            BundleViewEventDispatchError::InvalidDialogueAction {
                message: error.to_string(),
            }
        })? {
            RuntimeDialogueActionValue::None => Ok(None),
            RuntimeDialogueActionValue::Advance(target) => {
                Ok(Some(arcweft_view::DialogueAdvanceTarget::new(
                    arcweft_view::DialoguePresentationId::new(target.dialogue),
                    arcweft_view::DialogueEntryId::new(target.entry),
                    arcweft_view::DialogueInstanceId::new(target.instance),
                    arcweft_view::DialogueStageIndex::new(target.stage),
                    arcweft_view::DialogueRevision::new(target.revision),
                )))
            }
        }
    }
}

pub(crate) fn reconciled_root_handles_for_restore(
    handles: &[PresentationHandleRecord],
    dialogue: &[DialogueViewInput<'_>],
) -> Result<Vec<PresentationHandleRecord>, BundleViewAxisSeedError> {
    let mut reconciled = handles
        .iter()
        .cloned()
        .map(|record| (record.id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    for input in dialogue {
        let record = PresentationHandleRecord::new(
            input.handle.clone(),
            crate::presentation_handles::PresentationHandleKind::View,
            input.view.as_str().to_owned(),
            Some("dialogue".to_owned()),
            crate::presentation_handles::PresentationResourceState::Mounted,
            None,
            0,
        );
        if reconciled.insert(input.handle.clone(), record).is_some() {
            return Err(BundleViewAxisSeedError::SnapshotRootHandleCollision {
                handle: input.handle.clone(),
            });
        }
    }
    Ok(reconciled.into_values().collect())
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
    definition: &ViewId,
) -> u64 {
    let mut transcript = b"arcweft.view-mount.v1".to_vec();
    append_seed_part(&mut transcript, handle.as_str().as_bytes());
    append_seed_part(&mut transcript, definition.as_str().as_bytes());
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
