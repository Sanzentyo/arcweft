//! Portable, input-gated dialogue playback through authored View mounts.

mod store;

use crate::presentation_handles::PresentationHandleId;
use arcweft_core::plan::RuntimeLineId;
use arcweft_presentation::fx::{FxApplication, FxInstanceId};
use arcweft_render_text::{LineDisplayFrame, LineDisplayStage};
use serde::{Deserialize, Serialize};

pub use arcweft_view::{
    DialogueAdvanceTarget, DialogueEntryId, DialogueInstanceId, DialoguePresentationId,
    DialogueRevision, DialogueStageIndex,
};
pub use store::{DialoguePresentationStore, DialoguePresentationStoreError};

/// Canonical authored View selected when a dialogue line does not specify one.
pub const DEFAULT_DIALOGUE_VIEW: &str = arcweft_bundle::standard_view::DIALOGUE_VIEW_ID;

/// Authored View definition selected for a dialogue presentation.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DialogueViewDefinition(String);

impl DialogueViewDefinition {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for DialogueViewDefinition {
    fn default() -> Self {
        Self(DEFAULT_DIALOGUE_VIEW.to_owned())
    }
}

impl From<&str> for DialogueViewDefinition {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DialogueViewDefinition {
    fn from(value: String) -> Self {
        Self::new(value)
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

    pub(super) fn from_usize(value: usize) -> Option<Self> {
        u32::try_from(value).ok().map(Self)
    }
}

/// Stable identity record exposed to the authored `DialogueView` parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueViewOccurrence {
    pub presentation: DialoguePresentationId,
    pub entry: DialogueEntryId,
    pub instance: DialogueInstanceId,
}

/// Current input-gated page/stage record exposed to an authored dialogue View.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueViewStage {
    pub index: DialogueStageIndex,
    pub page: DialoguePageIndex,
    pub stage_count: u64,
    pub page_count: u64,
}

/// Deterministic logical reveal state exposed independently from renderer pixels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueViewReveal {
    /// Logical progress in thousandths, in the inclusive range `0..=1000`.
    pub progress_milli: u16,
    pub complete: bool,
}

impl DialogueViewReveal {
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            progress_milli: 0,
            complete: false,
        }
    }

    #[must_use]
    pub const fn complete() -> Self {
        Self {
            progress_milli: 1_000,
            complete: true,
        }
    }
}

/// Stale-safe primary action carried by the authored dialogue View parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueViewPrimaryAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<DialogueAdvanceTarget>,
}

/// Complete typed non-text state supplied to one authored dialogue View mount.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueViewState {
    pub occurrence: DialogueViewOccurrence,
    pub stage: DialogueViewStage,
    pub reveal: DialogueViewReveal,
    pub primary_action: DialogueViewPrimaryAction,
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
    UnknownPresentation,
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
        revision: DialogueRevision,
        from: DialogueStageIndex,
        to: DialogueStageIndex,
        page: DialoguePageIndex,
        advance: DialogueStageAdvanceKind,
    },
    RuntimeLineAdvanceQueued {
        target: DialogueAdvanceTarget,
        revision: DialogueRevision,
        line: RuntimeLineId,
    },
    DialogueAdvanceRejected {
        target: Option<DialogueAdvanceTarget>,
        reason: DialogueAdvanceRejection,
    },
}

/// Ordered update applied to one authored dialogue View target.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialoguePresentationOperation {
    Append {
        view: DialogueViewDefinition,
        frame: LineDisplayFrame,
    },
    Replace {
        view: DialogueViewDefinition,
        frame: LineDisplayFrame,
    },
    Clear {
        view: DialogueViewDefinition,
    },
}

impl DialoguePresentationOperation {
    #[must_use]
    pub fn append(view: impl Into<DialogueViewDefinition>, frame: LineDisplayFrame) -> Self {
        Self::Append {
            view: view.into(),
            frame,
        }
    }

    #[must_use]
    pub fn replace(view: impl Into<DialogueViewDefinition>, frame: LineDisplayFrame) -> Self {
        Self::Replace {
            view: view.into(),
            frame,
        }
    }

    #[must_use]
    pub fn clear(view: impl Into<DialogueViewDefinition>) -> Self {
        Self::Clear { view: view.into() }
    }
}

/// One retained dialogue occurrence evaluated as its own authored View mount.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DialogueEntryState {
    pub(super) id: DialogueEntryId,
    pub(super) instance: DialogueInstanceId,
    pub(super) stage: DialogueStageIndex,
    pub(super) waiting_for_advance: bool,
    pub(super) frame: LineDisplayFrame,
}

impl DialogueEntryState {
    #[must_use]
    pub const fn id(&self) -> DialogueEntryId {
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

    /// Stable root handle used by the shared authored View runtime.
    ///
    /// # Panics
    ///
    /// Panics only if the decimal representation of a `u64` ceases to satisfy
    /// the presentation-handle token grammar.
    #[must_use]
    pub fn view_handle_id(&self) -> PresentationHandleId {
        PresentationHandleId::try_new(format!("dialogue.{}", self.instance.get()))
            .expect("numeric dialogue occurrence ids form valid presentation handle ids")
    }

    /// Derives stage-independent identity for one Fx application in this occurrence.
    #[must_use]
    pub fn fx_instance_id(
        &self,
        dialogue: DialoguePresentationId,
        application: &FxApplication,
    ) -> FxInstanceId {
        let dialogue = format!("presentation.{}", dialogue.get());
        let entry = format!("entry.{}", self.id.get());
        let occurrence = format!("occurrence.{}", self.instance.get());
        let line = self.frame.line.canonical_label();
        application.derive_instance_id([
            "dialogue",
            line.as_str(),
            dialogue.as_str(),
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

    pub(super) fn advance_stage(
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

/// Persistent dialogue history attached to one authored View definition.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DialoguePresentation {
    pub(super) id: DialoguePresentationId,
    pub(super) view: DialogueViewDefinition,
    pub(super) revision: DialogueRevision,
    pub(super) entries: Vec<DialogueEntryState>,
    pub(super) active: Option<DialogueEntryId>,
}

impl DialoguePresentation {
    #[must_use]
    pub const fn id(&self) -> DialoguePresentationId {
        self.id
    }

    #[must_use]
    pub const fn view(&self) -> &DialogueViewDefinition {
        &self.view
    }

    #[must_use]
    pub const fn revision(&self) -> DialogueRevision {
        self.revision
    }

    #[must_use]
    pub fn entries(&self) -> &[DialogueEntryState] {
        &self.entries
    }

    #[must_use]
    pub const fn active_entry_id(&self) -> Option<DialogueEntryId> {
        self.active
    }

    #[must_use]
    pub fn active_entry(&self) -> Option<&DialogueEntryState> {
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

    pub(super) fn bump_revision(&mut self) -> Result<(), DialoguePresentationStoreError> {
        self.revision = self
            .revision
            .next()
            .ok_or(DialoguePresentationStoreError::RevisionExhausted { dialogue: self.id })?;
        Ok(())
    }
}

/// Borrowed typed input supplied to one authored dialogue View occurrence.
#[derive(Clone, Debug)]
pub struct DialogueViewInput<'a> {
    pub handle: PresentationHandleId,
    pub view: &'a str,
    pub frame: &'a LineDisplayFrame,
    pub state: DialogueViewState,
}
