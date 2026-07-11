//! Portable, input-gated dialogue playback state.

use arcweft_core::plan::RuntimeLineId;
use arcweft_render_text::{LineDisplayFrame, LineDisplayStage};
use serde::{Deserialize, Serialize};

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

    const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
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
    pub instance: DialogueInstanceId,
    pub stage: DialogueStageIndex,
}

impl DialogueAdvanceTarget {
    #[must_use]
    pub const fn new(instance: DialogueInstanceId, stage: DialogueStageIndex) -> Self {
        Self { instance, stage }
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
    NotWaiting,
    StaleInstance,
    StaleStage,
    InvalidStage,
    UntargetedRuntimeInput,
}

/// Deterministic result of routing one dialogue progression request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundlePresentationTransition {
    StageAdvanced {
        instance: DialogueInstanceId,
        from: DialogueStageIndex,
        to: DialogueStageIndex,
        page: DialoguePageIndex,
        advance: DialogueStageAdvanceKind,
    },
    RuntimeLineAdvanceQueued {
        target: DialogueAdvanceTarget,
        line: RuntimeLineId,
    },
    DialogueAdvanceRejected {
        target: Option<DialogueAdvanceTarget>,
        reason: DialogueAdvanceRejection,
    },
}

/// Portable presentation state for one retained dialogue textbox.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleDialoguePresentation {
    instance: DialogueInstanceId,
    stage: DialogueStageIndex,
    waiting_for_advance: bool,
    frame: LineDisplayFrame,
}

impl BundleDialoguePresentation {
    pub(crate) fn first(
        frame: LineDisplayFrame,
        instance: DialogueInstanceId,
        waiting_for_advance: bool,
    ) -> Self {
        Self {
            instance,
            stage: DialogueStageIndex::default(),
            waiting_for_advance,
            frame,
        }
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

    #[must_use]
    pub fn page_index(&self) -> Option<DialoguePageIndex> {
        self.current_stage()
            .and_then(|stage| DialoguePageIndex::from_usize(stage.page_index()))
    }

    /// Number of logical pages in this dialogue occurrence.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frame.page_count()
    }

    #[must_use]
    pub const fn is_waiting_for_advance(&self) -> bool {
        self.waiting_for_advance
    }

    #[must_use]
    pub fn advance_target(&self) -> Option<DialogueAdvanceTarget> {
        self.waiting_for_advance
            .then(|| DialogueAdvanceTarget::new(self.instance, self.stage))
    }

    pub(crate) fn next_instance(&self) -> DialogueInstanceId {
        self.instance.next()
    }

    pub(crate) fn set_waiting_for_advance(&mut self, waiting: bool) {
        self.waiting_for_advance = waiting;
    }

    pub(crate) fn validate_target(
        &self,
        target: DialogueAdvanceTarget,
    ) -> Result<(), DialogueAdvanceRejection> {
        if !self.waiting_for_advance {
            return Err(DialogueAdvanceRejection::NotWaiting);
        }
        if target.instance != self.instance {
            return Err(DialogueAdvanceRejection::StaleInstance);
        }
        if target.stage != self.stage {
            return Err(DialogueAdvanceRejection::StaleStage);
        }
        self.current_stage()
            .ok_or(DialogueAdvanceRejection::InvalidStage)
            .map(|_| ())
    }

    pub(crate) fn advance_stage(
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
