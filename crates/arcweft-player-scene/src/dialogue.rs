//! Shared dialogue reveal clock for interactive player hosts.

use arcweft_runtime_driver::dialogue::{
    DialogueStageIndex, TextBoxEntryId, TextBoxEntryState, TextBoxPresentation, TextBoxRuntimeId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DialogueVisualStage {
    textbox: TextBoxRuntimeId,
    entry: TextBoxEntryId,
    stage: DialogueStageIndex,
}

impl From<(&TextBoxPresentation, &TextBoxEntryState)> for DialogueVisualStage {
    fn from((textbox, entry): (&TextBoxPresentation, &TextBoxEntryState)) -> Self {
        Self {
            textbox: textbox.id(),
            entry: entry.id(),
            stage: entry.stage_index(),
        }
    }
}

/// Serializable reveal progress independent of a host's absolute clock.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueVisualClockSnapshot {
    active: Option<DialogueVisualStage>,
    elapsed_millis: u64,
    complete: bool,
}

/// Page/stage-aware reveal clock shared by native and Web players.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DialogueVisualClock {
    active: Option<DialogueVisualStage>,
    started_at_millis: u64,
    elapsed_offset_millis: u64,
    complete: bool,
}

/// Stage-local reveal progress without overloading elapsed time as a completion sentinel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DialogueVisualProgress {
    elapsed_millis: u64,
    complete: bool,
}

impl DialogueVisualProgress {
    #[must_use]
    pub const fn elapsed_millis(self) -> u64 {
        self.elapsed_millis
    }

    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.complete
    }
}

impl DialogueVisualClock {
    /// Returns stage-local progress, resetting on every occurrence/stage change.
    pub fn progress(
        &mut self,
        dialogue: Option<(&TextBoxPresentation, &TextBoxEntryState)>,
        now_millis: u64,
        override_millis: Option<u64>,
    ) -> DialogueVisualProgress {
        self.progress_for_stage(
            dialogue.map(DialogueVisualStage::from),
            now_millis,
            override_millis,
        )
    }

    fn progress_for_stage(
        &mut self,
        active: Option<DialogueVisualStage>,
        now_millis: u64,
        override_millis: Option<u64>,
    ) -> DialogueVisualProgress {
        let Some(active) = active else {
            self.active = None;
            self.started_at_millis = now_millis;
            self.elapsed_offset_millis = 0;
            self.complete = false;
            return DialogueVisualProgress::default();
        };
        if self.active != Some(active) {
            self.active = Some(active);
            self.started_at_millis = now_millis;
            self.elapsed_offset_millis = 0;
            self.complete = false;
        }
        DialogueVisualProgress {
            elapsed_millis: override_millis.unwrap_or_else(|| {
                self.elapsed_offset_millis
                    .saturating_add(now_millis.saturating_sub(self.started_at_millis))
            }),
            complete: self.complete,
        }
    }

    /// Completes only the currently observed stage, not the whole source line.
    pub fn complete_current_stage(&mut self) {
        if self.active.is_some() {
            self.complete = true;
        }
    }

    #[must_use]
    pub fn snapshot(&self, now_millis: u64) -> DialogueVisualClockSnapshot {
        DialogueVisualClockSnapshot {
            active: self.active,
            elapsed_millis: self
                .elapsed_offset_millis
                .saturating_add(now_millis.saturating_sub(self.started_at_millis)),
            complete: self.complete,
        }
    }

    pub fn restore(&mut self, snapshot: DialogueVisualClockSnapshot, now_millis: u64) {
        self.active = snapshot.active;
        self.started_at_millis = now_millis;
        self.elapsed_offset_millis = snapshot.elapsed_millis;
        self.complete = snapshot.complete;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn stage(index: u32) -> DialogueVisualStage {
        DialogueVisualStage {
            textbox: TextBoxRuntimeId::new(7),
            entry: TextBoxEntryId::new(11),
            stage: DialogueStageIndex::new(index),
        }
    }

    #[test]
    fn occurrence_and_stage_key_reset_completion() {
        let mut clock = DialogueVisualClock::default();
        assert_eq!(
            clock.progress_for_stage(Some(stage(0)), 100, None),
            DialogueVisualProgress::default()
        );
        clock.complete_current_stage();
        let completed = clock.progress_for_stage(Some(stage(0)), 110, None);
        assert_eq!(completed.elapsed_millis(), 10);
        assert!(completed.is_complete());

        assert_eq!(
            clock.progress_for_stage(Some(stage(1)), 120, None),
            DialogueVisualProgress::default()
        );
    }

    #[test]
    fn snapshot_restores_elapsed_time_without_absolute_clock() {
        let mut clock = DialogueVisualClock::default();
        assert_eq!(
            clock.progress_for_stage(Some(stage(0)), 100, None),
            DialogueVisualProgress::default()
        );
        let snapshot = clock.snapshot(140);

        let mut restored = DialogueVisualClock::default();
        restored.restore(snapshot, 1_000);
        assert_eq!(
            restored
                .progress_for_stage(Some(stage(0)), 1_010, None)
                .elapsed_millis(),
            50
        );
    }
}
