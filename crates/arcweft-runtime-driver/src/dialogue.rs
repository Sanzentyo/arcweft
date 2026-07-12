//! Portable, input-gated dialogue playback and persistent `TextBox` state.

use arcweft_core::plan::RuntimeLineId;
use arcweft_presentation::fx::{FxApplication, FxInstanceId};
use arcweft_render_text::{LineDisplayFrame, LineDisplayStage};
use arcweft_view::{ViewMountAllocationError, ViewMountAllocator, ViewMountId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Canonical target used when a dialogue line does not select a window.
pub const DEFAULT_TEXTBOX_TARGET: &str = "textbox.main";

/// Monotonic identity for one presentation of a dialogue line.
///
/// The same source line can execute repeatedly, so `RuntimeLineId` alone is
/// not sufficient to reset page and reveal state.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialogueInstanceId(u64);

impl DialogueInstanceId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable runtime identity for one persistent `TextBox` target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TextBoxRuntimeId(u64);

impl TextBoxRuntimeId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable identity for one retained `TextBox` entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TextBoxEntryId(u64);

impl TextBoxEntryId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic mutation revision for one persistent `TextBox`.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct TextBoxRevision(u64);

impl TextBoxRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Typed mount namespace for the Rust-backed View implementing a `TextBox`.
///
/// The wrapper keeps `TextBox` mount identity distinct from authored View mount
/// identity while using the same allocator and persisted representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TextBoxViewMountId(ViewMountId);

impl TextBoxViewMountId {
    #[must_use]
    pub const fn view_mount_id(self) -> ViewMountId {
        self.0
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Canonical public target of a persistent `TextBox`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TextBoxTargetId(String);

impl TextBoxTargetId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TextBoxTargetId {
    fn default() -> Self {
        Self(DEFAULT_TEXTBOX_TARGET.to_owned())
    }
}

impl From<&str> for TextBoxTargetId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TextBoxTargetId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Zero-based input-gated stage within one dialogue occurrence.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialogueStageIndex(u32);

impl DialogueStageIndex {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    fn as_usize(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Zero-based logical page within one dialogue occurrence.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialoguePageIndex(u32);

impl DialoguePageIndex {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    fn from_usize(value: usize) -> Option<Self> {
        u32::try_from(value).ok().map(Self)
    }
}

/// Stable target captured when a host requests dialogue progression.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueAdvanceTarget {
    pub textbox: TextBoxRuntimeId,
    pub entry: TextBoxEntryId,
    pub instance: DialogueInstanceId,
    pub stage: DialogueStageIndex,
    pub revision: TextBoxRevision,
}

impl DialogueAdvanceTarget {
    #[must_use]
    pub const fn new(
        textbox: TextBoxRuntimeId,
        entry: TextBoxEntryId,
        instance: DialogueInstanceId,
        stage: DialogueStageIndex,
        revision: TextBoxRevision,
    ) -> Self {
        Self {
            textbox,
            entry,
            instance,
            stage,
            revision,
        }
    }
}

/// Presentation-level input consumed before runtime input routing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundlePresentationInput {
    AdvanceDialogue { target: DialogueAdvanceTarget },
}

impl BundlePresentationInput {
    #[must_use]
    pub const fn advance_dialogue(target: DialogueAdvanceTarget) -> Self {
        Self::AdvanceDialogue { target }
    }
}

/// How an internal dialogue stage transition changes the visible page.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueStageAdvanceKind {
    ContinuePage,
    NextPage,
}

/// Reason a targeted dialogue advance was not accepted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueAdvanceRejection {
    NoDialogue,
    UnknownTextBox,
    StaleEntry,
    StaleRevision,
    NotWaiting,
    StaleInstance,
    StaleStage,
    InvalidStage,
    RevisionExhausted,
    UntargetedRuntimeInput,
}

/// Deterministic result of routing one dialogue progression request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundlePresentationTransition {
    StageAdvanced {
        target: DialogueAdvanceTarget,
        revision: TextBoxRevision,
        from: DialogueStageIndex,
        to: DialogueStageIndex,
        page: DialoguePageIndex,
        advance: DialogueStageAdvanceKind,
    },
    RuntimeLineAdvanceQueued {
        target: DialogueAdvanceTarget,
        revision: TextBoxRevision,
        line: RuntimeLineId,
    },
    DialogueAdvanceRejected {
        target: Option<DialogueAdvanceTarget>,
        reason: DialogueAdvanceRejection,
    },
}

/// Ordered operation applied to one persistent `TextBox` target.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextBoxPresentationOperation {
    Append {
        target: TextBoxTargetId,
        frame: LineDisplayFrame,
    },
    Replace {
        target: TextBoxTargetId,
        frame: LineDisplayFrame,
    },
    Clear {
        target: TextBoxTargetId,
    },
}

impl TextBoxPresentationOperation {
    #[must_use]
    pub fn append(target: impl Into<TextBoxTargetId>, frame: LineDisplayFrame) -> Self {
        Self::Append {
            target: target.into(),
            frame,
        }
    }

    #[must_use]
    pub fn replace(target: impl Into<TextBoxTargetId>, frame: LineDisplayFrame) -> Self {
        Self::Replace {
            target: target.into(),
            frame,
        }
    }

    #[must_use]
    pub fn clear(target: impl Into<TextBoxTargetId>) -> Self {
        Self::Clear {
            target: target.into(),
        }
    }
}

/// One retained dialogue entry in a persistent `TextBox`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextBoxEntryState {
    id: TextBoxEntryId,
    instance: DialogueInstanceId,
    stage: DialogueStageIndex,
    waiting_for_advance: bool,
    frame: LineDisplayFrame,
}

impl TextBoxEntryState {
    #[must_use]
    pub const fn id(&self) -> TextBoxEntryId {
        self.id
    }

    #[must_use]
    pub const fn instance(&self) -> DialogueInstanceId {
        self.instance
    }

    #[must_use]
    pub const fn stage_index(&self) -> DialogueStageIndex {
        self.stage
    }

    #[must_use]
    pub const fn frame(&self) -> &LineDisplayFrame {
        &self.frame
    }

    #[must_use]
    pub fn current_stage(&self) -> Option<LineDisplayStage<'_>> {
        self.stage
            .as_usize()
            .and_then(|index| self.frame.stage(index))
    }

    /// Derives stage-independent identity for one application in this entry.
    #[must_use]
    pub fn fx_instance_id(
        &self,
        textbox: TextBoxRuntimeId,
        application: &FxApplication,
    ) -> FxInstanceId {
        let textbox = format!("textbox.{}", textbox.get());
        let entry = format!("entry.{}", self.id.get());
        let occurrence = format!("occurrence.{}", self.instance.get());
        let line = self.frame.line.canonical_label();
        application.derive_instance_id([
            "dialogue",
            line.as_str(),
            textbox.as_str(),
            entry.as_str(),
            occurrence.as_str(),
        ])
    }

    #[must_use]
    pub fn page_index(&self) -> Option<DialoguePageIndex> {
        self.current_stage()
            .and_then(|stage| DialoguePageIndex::from_usize(stage.page_index()))
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frame.page_count()
    }

    #[must_use]
    pub const fn is_waiting_for_advance(&self) -> bool {
        self.waiting_for_advance
    }

    fn advance_stage(
        &mut self,
    ) -> Option<(
        DialogueStageIndex,
        DialogueStageIndex,
        DialoguePageIndex,
        DialogueStageAdvanceKind,
    )> {
        let from = self.stage;
        let from_page = self.page_index()?;
        let to = from.next()?;
        let next_stage = to.as_usize().and_then(|index| self.frame.stage(index))?;
        let page = DialoguePageIndex::from_usize(next_stage.page_index())?;
        self.stage = to;
        let advance = if page == from_page {
            DialogueStageAdvanceKind::ContinuePage
        } else {
            DialogueStageAdvanceKind::NextPage
        };
        Some((from, to, page, advance))
    }
}

/// Persistent state for one `TextBox` target and its Rust-backed View mount.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextBoxPresentation {
    id: TextBoxRuntimeId,
    target: TextBoxTargetId,
    revision: TextBoxRevision,
    entries: Vec<TextBoxEntryState>,
    active: Option<TextBoxEntryId>,
    mount: TextBoxViewMountId,
}

impl TextBoxPresentation {
    #[must_use]
    pub const fn id(&self) -> TextBoxRuntimeId {
        self.id
    }

    #[must_use]
    pub const fn target(&self) -> &TextBoxTargetId {
        &self.target
    }

    #[must_use]
    pub const fn revision(&self) -> TextBoxRevision {
        self.revision
    }

    #[must_use]
    pub fn entries(&self) -> &[TextBoxEntryState] {
        &self.entries
    }

    #[must_use]
    pub const fn active_entry_id(&self) -> Option<TextBoxEntryId> {
        self.active
    }

    #[must_use]
    pub const fn mount(&self) -> TextBoxViewMountId {
        self.mount
    }

    #[must_use]
    pub fn active_entry(&self) -> Option<&TextBoxEntryState> {
        let active = self.active?;
        self.entries.iter().find(|entry| entry.id == active)
    }

    #[must_use]
    pub fn advance_target(&self) -> Option<DialogueAdvanceTarget> {
        let entry = self.active_entry()?;
        entry.waiting_for_advance.then(|| {
            DialogueAdvanceTarget::new(
                self.id,
                entry.id,
                entry.instance,
                entry.stage,
                self.revision,
            )
        })
    }

    fn bump_revision(&mut self) -> Result<(), TextBoxStoreError> {
        self.revision = self
            .revision
            .next()
            .ok_or(TextBoxStoreError::RevisionExhausted { textbox: self.id })?;
        Ok(())
    }
}

/// Runtime-driver-owned persistent state for every independently targeted `TextBox`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TextBoxPresentationStore {
    textboxes: BTreeMap<TextBoxRuntimeId, TextBoxPresentation>,
    targets: BTreeMap<TextBoxTargetId, TextBoxRuntimeId>,
    next_textbox_id: u64,
    next_entry_id: u64,
    next_dialogue_instance_id: u64,
}

/// Failure to apply or restore persistent `TextBox` state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextBoxStoreError {
    #[error("TextBox {identity} identity allocator is exhausted")]
    IdentityExhausted { identity: &'static str },
    #[error("TextBox {textbox:?} revision is exhausted")]
    RevisionExhausted { textbox: TextBoxRuntimeId },
    #[error(transparent)]
    MountAllocation(#[from] ViewMountAllocationError),
    #[error("invalid TextBox presentation snapshot: {message}")]
    InvalidSnapshot { message: String },
}

enum TextBoxAdvanceOutcome {
    Stage {
        revision: TextBoxRevision,
        from: DialogueStageIndex,
        to: DialogueStageIndex,
        page: DialoguePageIndex,
        advance: DialogueStageAdvanceKind,
    },
    Line {
        revision: TextBoxRevision,
        line: RuntimeLineId,
    },
}

impl TextBoxPresentationStore {
    #[must_use]
    pub fn len(&self) -> usize {
        self.textboxes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textboxes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TextBoxPresentation> {
        self.textboxes.values()
    }

    #[must_use]
    pub fn get(&self, id: TextBoxRuntimeId) -> Option<&TextBoxPresentation> {
        self.textboxes.get(&id)
    }

    #[must_use]
    pub fn get_by_target(&self, target: &TextBoxTargetId) -> Option<&TextBoxPresentation> {
        self.targets
            .get(target)
            .and_then(|id| self.textboxes.get(id))
    }

    /// Returns the newest active dialogue occurrence across all targets.
    #[must_use]
    pub fn latest_active(&self) -> Option<(&TextBoxPresentation, &TextBoxEntryState)> {
        self.textboxes
            .values()
            .filter_map(|textbox| textbox.active_entry().map(|entry| (textbox, entry)))
            .max_by_key(|(_, entry)| entry.instance)
    }

    pub fn waiting_entries(
        &self,
    ) -> impl Iterator<Item = (&TextBoxPresentation, &TextBoxEntryState)> {
        self.textboxes.values().filter_map(|textbox| {
            textbox
                .active_entry()
                .filter(|entry| entry.waiting_for_advance)
                .map(|entry| (textbox, entry))
        })
    }

    pub(crate) fn view_mount_ids(&self) -> impl Iterator<Item = ViewMountId> + '_ {
        self.textboxes
            .values()
            .map(|textbox| textbox.mount.view_mount_id())
    }

    /// Applies append, replace, and clear operations in authored order.
    ///
    /// Allocation or revision failure leaves the live store unchanged.
    pub fn apply_operations(
        &mut self,
        operations: &[TextBoxPresentationOperation],
        mount_allocator: &mut ViewMountAllocator,
    ) -> Result<(), TextBoxStoreError> {
        let mut next = self.clone();
        let mut next_mount_allocator = *mount_allocator;
        for operation in operations {
            next.apply_operation(operation, &mut next_mount_allocator)?;
        }
        next.validate()?;
        *self = next;
        *mount_allocator = next_mount_allocator;
        Ok(())
    }

    /// Selects the newest occurrence matching the runtime's waiting line and
    /// clears actionable state from every other target.
    pub fn synchronize_waiting_line(
        &mut self,
        line: Option<&RuntimeLineId>,
    ) -> Result<(), TextBoxStoreError> {
        let selected = line.and_then(|line| {
            self.textboxes
                .values()
                .flat_map(|textbox| {
                    textbox.entries.iter().filter_map(move |entry| {
                        (&entry.frame.line == line).then_some((
                            textbox.id,
                            entry.id,
                            entry.instance,
                        ))
                    })
                })
                .max_by_key(|(_, _, instance)| *instance)
                .map(|(textbox, entry, _)| (textbox, entry))
        });
        let mut next = self.clone();
        for textbox in next.textboxes.values_mut() {
            let selected_entry = selected
                .filter(|(selected_textbox, _)| *selected_textbox == textbox.id)
                .map(|(_, entry)| entry);
            let mut changed = false;
            if let Some(entry) = selected_entry
                && textbox.active != Some(entry)
            {
                textbox.active = Some(entry);
                changed = true;
            }
            for entry in &mut textbox.entries {
                let waiting = selected_entry == Some(entry.id);
                if entry.waiting_for_advance != waiting {
                    entry.waiting_for_advance = waiting;
                    changed = true;
                }
            }
            if changed {
                textbox.bump_revision()?;
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn advance_dialogue(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> (
        BundlePresentationTransition,
        Option<arcweft_core::plan::RuntimeLineId>,
    ) {
        let outcome = self.advance_target(target);
        match outcome {
            Ok(TextBoxAdvanceOutcome::Stage {
                revision,
                from,
                to,
                page,
                advance,
            }) => (
                BundlePresentationTransition::StageAdvanced {
                    target,
                    revision,
                    from,
                    to,
                    page,
                    advance,
                },
                None,
            ),
            Ok(TextBoxAdvanceOutcome::Line { revision, line }) => (
                BundlePresentationTransition::RuntimeLineAdvanceQueued {
                    target,
                    revision,
                    line: line.clone(),
                },
                Some(line),
            ),
            Err(reason) => (
                BundlePresentationTransition::DialogueAdvanceRejected {
                    target: Some(target),
                    reason,
                },
                None,
            ),
        }
    }

    /// Validates all persisted indexes, allocator cursors, frames, and active entries.
    pub fn validate(&self) -> Result<(), TextBoxStoreError> {
        if self.targets.len() != self.textboxes.len() {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: "target index and TextBox table have different lengths".to_owned(),
            });
        }
        let mut entry_ids = BTreeSet::new();
        let mut instances = BTreeSet::new();
        let mut mounts = BTreeSet::new();
        for (id, textbox) in &self.textboxes {
            self.validate_textbox_record(
                *id,
                textbox,
                &mut entry_ids,
                &mut instances,
                &mut mounts,
            )?;
        }
        Self::validate_cursor(
            "runtime",
            self.next_textbox_id,
            self.textboxes.keys().map(|id| id.get()).max(),
        )?;
        Self::validate_cursor(
            "entry",
            self.next_entry_id,
            entry_ids.iter().map(|id| id.get()).max(),
        )?;
        Self::validate_cursor(
            "dialogue occurrence",
            self.next_dialogue_instance_id,
            instances.iter().map(|id| id.get()).max(),
        )?;
        Ok(())
    }

    fn validate_textbox_record(
        &self,
        id: TextBoxRuntimeId,
        textbox: &TextBoxPresentation,
        entry_ids: &mut BTreeSet<TextBoxEntryId>,
        instances: &mut BTreeSet<DialogueInstanceId>,
        mounts: &mut BTreeSet<TextBoxViewMountId>,
    ) -> Result<(), TextBoxStoreError> {
        if id != textbox.id {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox table key {id:?} does not match its stored id"),
            });
        }
        if textbox.target.as_str().is_empty() {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox {id:?} has an empty target"),
            });
        }
        if self.targets.get(&textbox.target) != Some(&id) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox {id:?} is missing from its target index"),
            });
        }
        if !mounts.insert(textbox.mount) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox mount {:?} appears more than once", textbox.mount),
            });
        }
        if textbox.entries.is_empty() != textbox.active.is_none() {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!(
                    "TextBox {id:?} must have exactly one active entry when its entry list is non-empty"
                ),
            });
        }
        if textbox
            .active
            .is_some_and(|active| !textbox.entries.iter().any(|entry| entry.id == active))
        {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox {id:?} has an unknown active entry"),
            });
        }
        for entry in &textbox.entries {
            Self::validate_entry_record(textbox, entry, entry_ids, instances)?;
        }
        Ok(())
    }

    fn validate_entry_record(
        textbox: &TextBoxPresentation,
        entry: &TextBoxEntryState,
        entry_ids: &mut BTreeSet<TextBoxEntryId>,
        instances: &mut BTreeSet<DialogueInstanceId>,
    ) -> Result<(), TextBoxStoreError> {
        if !entry_ids.insert(entry.id) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox entry {:?} appears more than once", entry.id),
            });
        }
        if !instances.insert(entry.instance) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue occurrence {:?} appears more than once",
                    entry.instance
                ),
            });
        }
        entry
            .frame
            .validate()
            .map_err(|error| TextBoxStoreError::InvalidSnapshot {
                message: format!("TextBox entry {:?} has an invalid frame: {error}", entry.id),
            })?;
        if entry.current_stage().is_none() {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!(
                    "TextBox entry {:?} has out-of-range stage {} for {} stage(s)",
                    entry.id,
                    entry.stage.get(),
                    entry.frame.stage_count(),
                ),
            });
        }
        if entry.waiting_for_advance && textbox.active != Some(entry.id) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!("inactive TextBox entry {:?} is marked actionable", entry.id),
            });
        }
        Ok(())
    }

    fn apply_operation(
        &mut self,
        operation: &TextBoxPresentationOperation,
        mount_allocator: &mut ViewMountAllocator,
    ) -> Result<(), TextBoxStoreError> {
        match operation {
            TextBoxPresentationOperation::Append { target, frame } => {
                self.insert_entry(target, frame, false, mount_allocator)
            }
            TextBoxPresentationOperation::Replace { target, frame } => {
                self.insert_entry(target, frame, true, mount_allocator)
            }
            TextBoxPresentationOperation::Clear { target } => {
                self.clear_target(target, mount_allocator)
            }
        }
    }

    fn insert_entry(
        &mut self,
        target: &TextBoxTargetId,
        frame: &LineDisplayFrame,
        replace: bool,
        mount_allocator: &mut ViewMountAllocator,
    ) -> Result<(), TextBoxStoreError> {
        let textbox = self.ensure_textbox(target, mount_allocator)?;
        let entry_id = TextBoxEntryId(Self::allocate_identity(&mut self.next_entry_id, "entry")?);
        let instance = DialogueInstanceId(Self::allocate_identity(
            &mut self.next_dialogue_instance_id,
            "dialogue occurrence",
        )?);
        let entry = TextBoxEntryState {
            id: entry_id,
            instance,
            stage: DialogueStageIndex::default(),
            waiting_for_advance: false,
            frame: frame.clone(),
        };
        let presentation = self
            .textboxes
            .get_mut(&textbox)
            .expect("target index was created with its TextBox presentation");
        if replace {
            presentation.entries.clear();
        }
        presentation.entries.push(entry);
        presentation.active = Some(entry_id);
        presentation.bump_revision()
    }

    fn clear_target(
        &mut self,
        target: &TextBoxTargetId,
        mount_allocator: &mut ViewMountAllocator,
    ) -> Result<(), TextBoxStoreError> {
        let textbox = self.ensure_textbox(target, mount_allocator)?;
        let presentation = self
            .textboxes
            .get_mut(&textbox)
            .expect("target index was created with its TextBox presentation");
        presentation.entries.clear();
        presentation.active = None;
        presentation.bump_revision()
    }

    fn ensure_textbox(
        &mut self,
        target: &TextBoxTargetId,
        mount_allocator: &mut ViewMountAllocator,
    ) -> Result<TextBoxRuntimeId, TextBoxStoreError> {
        if let Some(id) = self.targets.get(target) {
            return Ok(*id);
        }
        let id = TextBoxRuntimeId(Self::allocate_identity(
            &mut self.next_textbox_id,
            "runtime",
        )?);
        let mount = TextBoxViewMountId(mount_allocator.allocate()?);
        let presentation = TextBoxPresentation {
            id,
            target: target.clone(),
            revision: TextBoxRevision::default(),
            entries: Vec::new(),
            active: None,
            mount,
        };
        self.targets.insert(target.clone(), id);
        self.textboxes.insert(id, presentation);
        Ok(id)
    }

    fn advance_target(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> Result<TextBoxAdvanceOutcome, DialogueAdvanceRejection> {
        if self.textboxes.is_empty() {
            return Err(DialogueAdvanceRejection::NoDialogue);
        }
        let textbox = self
            .textboxes
            .get_mut(&target.textbox)
            .ok_or(DialogueAdvanceRejection::UnknownTextBox)?;
        if target.revision != textbox.revision {
            return Err(DialogueAdvanceRejection::StaleRevision);
        }
        if textbox.revision.next().is_none() {
            return Err(DialogueAdvanceRejection::RevisionExhausted);
        }
        if textbox.active != Some(target.entry) {
            return Err(DialogueAdvanceRejection::StaleEntry);
        }
        let entry = textbox
            .entries
            .iter_mut()
            .find(|entry| entry.id == target.entry)
            .ok_or(DialogueAdvanceRejection::StaleEntry)?;
        if !entry.waiting_for_advance {
            return Err(DialogueAdvanceRejection::NotWaiting);
        }
        if target.instance != entry.instance {
            return Err(DialogueAdvanceRejection::StaleInstance);
        }
        if target.stage != entry.stage {
            return Err(DialogueAdvanceRejection::StaleStage);
        }
        if entry.current_stage().is_none() {
            return Err(DialogueAdvanceRejection::InvalidStage);
        }
        if let Some((from, to, page, advance)) = entry.advance_stage() {
            textbox
                .bump_revision()
                .map_err(|_| DialogueAdvanceRejection::RevisionExhausted)?;
            return Ok(TextBoxAdvanceOutcome::Stage {
                revision: textbox.revision,
                from,
                to,
                page,
                advance,
            });
        }
        let line = entry.frame.line.clone();
        entry.waiting_for_advance = false;
        textbox
            .bump_revision()
            .map_err(|_| DialogueAdvanceRejection::RevisionExhausted)?;
        Ok(TextBoxAdvanceOutcome::Line {
            revision: textbox.revision,
            line,
        })
    }

    fn allocate_identity(
        cursor: &mut u64,
        identity: &'static str,
    ) -> Result<u64, TextBoxStoreError> {
        let allocated = *cursor;
        *cursor = cursor
            .checked_add(1)
            .ok_or(TextBoxStoreError::IdentityExhausted { identity })?;
        Ok(allocated)
    }

    fn validate_cursor(
        identity: &'static str,
        cursor: u64,
        greatest_live: Option<u64>,
    ) -> Result<(), TextBoxStoreError> {
        if greatest_live.is_some_and(|greatest| cursor <= greatest) {
            return Err(TextBoxStoreError::InvalidSnapshot {
                message: format!(
                    "TextBox {identity} cursor {cursor} is not newer than live identity {}",
                    greatest_live.expect("checked as present")
                ),
            });
        }
        Ok(())
    }
}
