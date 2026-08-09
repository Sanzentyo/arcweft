use super::{
    BundlePresentationTransition, DialogueAdvanceRejection, DialogueAdvanceTarget, DialogueEntryId,
    DialogueEntryState, DialogueInstanceId, DialoguePageIndex, DialoguePresentation,
    DialoguePresentationId, DialoguePresentationOperation, DialogueRevision,
    DialogueStageAdvanceKind, DialogueStageIndex, DialogueViewDefinition, DialogueViewInput,
    DialogueViewOccurrence, DialogueViewPrimaryAction, DialogueViewReveal, DialogueViewStage,
    DialogueViewState,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_text_model::LineDisplayFrame;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Runtime-driver-owned state for independently targeted authored dialogue Views.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DialoguePresentationStore {
    presentations: BTreeMap<DialoguePresentationId, DialoguePresentation>,
    definitions: BTreeMap<DialogueViewDefinition, DialoguePresentationId>,
    next_presentation_id: u64,
    next_entry_id: u64,
    next_dialogue_instance_id: u64,
}

/// Failure to apply or restore persistent dialogue View state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DialoguePresentationStoreError {
    #[error("dialogue {identity} identity allocator is exhausted")]
    IdentityExhausted { identity: &'static str },
    #[error("dialogue presentation {dialogue:?} revision is exhausted")]
    RevisionExhausted { dialogue: DialoguePresentationId },
    #[error("invalid dialogue presentation snapshot: {message}")]
    InvalidSnapshot { message: String },
}

enum DialogueAdvanceOutcome {
    Stage {
        revision: DialogueRevision,
        from: DialogueStageIndex,
        to: DialogueStageIndex,
        page: DialoguePageIndex,
        advance: DialogueStageAdvanceKind,
    },
    Line {
        revision: DialogueRevision,
        line: RuntimeLineId,
    },
}

impl DialoguePresentationStore {
    #[must_use]
    pub fn len(&self) -> usize {
        self.presentations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.presentations.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DialoguePresentation> {
        self.presentations.values()
    }

    #[must_use]
    pub fn get(&self, id: DialoguePresentationId) -> Option<&DialoguePresentation> {
        self.presentations.get(&id)
    }

    #[must_use]
    pub fn get_by_definition(
        &self,
        definition: &DialogueViewDefinition,
    ) -> Option<&DialoguePresentation> {
        self.definitions
            .get(definition)
            .and_then(|id| self.presentations.get(id))
    }

    /// Returns the newest active dialogue occurrence across all authored Views.
    #[must_use]
    pub fn latest_active(&self) -> Option<(&DialoguePresentation, &DialogueEntryState)> {
        self.presentations
            .values()
            .filter_map(|dialogue| dialogue.active_entry().map(|entry| (dialogue, entry)))
            .max_by_key(|(_, entry)| entry.instance)
    }

    pub fn waiting_entries(
        &self,
    ) -> impl Iterator<Item = (&DialoguePresentation, &DialogueEntryState)> {
        self.presentations.values().filter_map(|dialogue| {
            dialogue
                .active_entry()
                .filter(|entry| entry.waiting_for_advance)
                .map(|entry| (dialogue, entry))
        })
    }

    /// Supplies every retained occurrence to the shared authored View evaluator.
    ///
    /// # Panics
    ///
    /// Panics only on a Rust target whose addressable `usize` range exceeds
    /// the serialized `u64` count contract.
    pub fn view_inputs(&self) -> Vec<DialogueViewInput<'_>> {
        self.presentations
            .values()
            .filter_map(|dialogue| {
                let entry = dialogue.active_entry()?;
                let stage = entry.current_stage()?;
                Some(DialogueViewInput {
                    handle: entry.view_handle_id(),
                    view: dialogue.view.view_id(),
                    frame: &entry.frame,
                    state: DialogueViewState {
                        occurrence: DialogueViewOccurrence {
                            presentation: dialogue.id,
                            entry: entry.id,
                            instance: entry.instance,
                        },
                        stage: DialogueViewStage {
                            index: entry.stage,
                            page: DialoguePageIndex::from_usize(stage.page_index())?,
                            stage_count: u64::try_from(entry.frame.stage_count())
                                .expect("View runtime targets have at most u64 addressable stages"),
                            page_count: u64::try_from(entry.frame.page_count())
                                .expect("View runtime targets have at most u64 addressable pages"),
                        },
                        reveal: if entry.waiting_for_advance {
                            DialogueViewReveal::complete()
                        } else {
                            DialogueViewReveal::pending()
                        },
                        primary_action: DialogueViewPrimaryAction {
                            target: dialogue.advance_target(),
                        },
                    },
                })
            })
            .collect()
    }

    /// Applies append, replace, and clear operations in authored order.
    ///
    /// Identity or revision failure leaves the live store unchanged.
    pub fn apply_operations(
        &mut self,
        operations: &[DialoguePresentationOperation],
    ) -> Result<(), DialoguePresentationStoreError> {
        let mut next = self.clone();
        for operation in operations {
            next.apply_operation(operation)?;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Selects the newest occurrence matching the runtime's waiting line and
    /// clears actionable state from every other authored dialogue View.
    pub fn synchronize_waiting_line(
        &mut self,
        line: Option<&RuntimeLineId>,
    ) -> Result<(), DialoguePresentationStoreError> {
        let selected = line.and_then(|line| {
            self.presentations
                .values()
                .flat_map(|dialogue| {
                    dialogue.entries.iter().filter_map(move |entry| {
                        (&entry.frame.line == line).then_some((
                            dialogue.id,
                            entry.id,
                            entry.instance,
                        ))
                    })
                })
                .max_by_key(|(_, _, instance)| *instance)
                .map(|(dialogue, entry, _)| (dialogue, entry))
        });
        let mut next = self.clone();
        for dialogue in next.presentations.values_mut() {
            let selected_entry = selected
                .filter(|(selected_dialogue, _)| *selected_dialogue == dialogue.id)
                .map(|(_, entry)| entry);
            let mut changed = false;
            if let Some(entry) = selected_entry
                && dialogue.active != Some(entry)
            {
                dialogue.active = Some(entry);
                changed = true;
            }
            for entry in &mut dialogue.entries {
                let waiting = selected_entry == Some(entry.id);
                if entry.waiting_for_advance != waiting {
                    entry.waiting_for_advance = waiting;
                    changed = true;
                }
            }
            if changed {
                dialogue.bump_revision()?;
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn advance_dialogue(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> (BundlePresentationTransition, Option<RuntimeLineId>) {
        match self.advance_target(target) {
            Ok(DialogueAdvanceOutcome::Stage {
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
            Ok(DialogueAdvanceOutcome::Line { revision, line }) => (
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

    /// Validates persisted indexes, allocator cursors, frames, and active entries.
    pub fn validate(&self) -> Result<(), DialoguePresentationStoreError> {
        if self.definitions.len() != self.presentations.len() {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message:
                    "View definition index and dialogue presentation table have different lengths"
                        .to_owned(),
            });
        }
        let mut entry_ids = BTreeSet::new();
        let mut instances = BTreeSet::new();
        for (id, dialogue) in &self.presentations {
            self.validate_presentation_record(*id, dialogue, &mut entry_ids, &mut instances)?;
        }
        Self::validate_cursor(
            "presentation",
            self.next_presentation_id,
            self.presentations.keys().map(|id| id.get()).max(),
        )?;
        Self::validate_cursor(
            "entry",
            self.next_entry_id,
            entry_ids.iter().map(|id| id.get()).max(),
        )?;
        Self::validate_cursor(
            "occurrence",
            self.next_dialogue_instance_id,
            instances.iter().map(|id| id.get()).max(),
        )
    }

    fn validate_presentation_record(
        &self,
        id: DialoguePresentationId,
        dialogue: &DialoguePresentation,
        entry_ids: &mut BTreeSet<DialogueEntryId>,
        instances: &mut BTreeSet<DialogueInstanceId>,
    ) -> Result<(), DialoguePresentationStoreError> {
        if id != dialogue.id {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue table key {id:?} does not match its stored presentation id"
                ),
            });
        }
        if self.definitions.get(&dialogue.view) != Some(&id) {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue presentation {id:?} is missing from its View definition index"
                ),
            });
        }
        if dialogue.entries.is_empty() != dialogue.active.is_none() {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue presentation {id:?} must have exactly one active entry when non-empty"
                ),
            });
        }
        if dialogue
            .active
            .is_some_and(|active| !dialogue.entries.iter().any(|entry| entry.id == active))
        {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!("dialogue presentation {id:?} has an unknown active entry"),
            });
        }
        for entry in &dialogue.entries {
            Self::validate_entry_record(dialogue, entry, entry_ids, instances)?;
        }
        Ok(())
    }

    fn validate_entry_record(
        dialogue: &DialoguePresentation,
        entry: &DialogueEntryState,
        entry_ids: &mut BTreeSet<DialogueEntryId>,
        instances: &mut BTreeSet<DialogueInstanceId>,
    ) -> Result<(), DialoguePresentationStoreError> {
        if !entry_ids.insert(entry.id) {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!("dialogue entry {:?} appears more than once", entry.id),
            });
        }
        if !instances.insert(entry.instance) {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue occurrence {:?} appears more than once",
                    entry.instance
                ),
            });
        }
        entry.frame.validate().map_err(|error| {
            DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue entry {:?} has an invalid frame: {error}",
                    entry.id
                ),
            }
        })?;
        if entry.current_stage().is_none() {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue entry {:?} has out-of-range stage {} for {} stage(s)",
                    entry.id,
                    entry.stage.get(),
                    entry.frame.stage_count(),
                ),
            });
        }
        if entry.waiting_for_advance && dialogue.active != Some(entry.id) {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "inactive dialogue entry {:?} is marked actionable",
                    entry.id
                ),
            });
        }
        Ok(())
    }

    fn apply_operation(
        &mut self,
        operation: &DialoguePresentationOperation,
    ) -> Result<(), DialoguePresentationStoreError> {
        match operation {
            DialoguePresentationOperation::Append { view, frame } => {
                self.insert_entry(view, frame, false)
            }
            DialoguePresentationOperation::Replace { view, frame } => {
                self.insert_entry(view, frame, true)
            }
            DialoguePresentationOperation::Clear { view } => self.clear_view(view),
        }
    }

    fn insert_entry(
        &mut self,
        view: &DialogueViewDefinition,
        frame: &LineDisplayFrame,
        replace: bool,
    ) -> Result<(), DialoguePresentationStoreError> {
        let presentation_id = self.ensure_presentation(view)?;
        let entry_id =
            DialogueEntryId::new(Self::allocate_identity(&mut self.next_entry_id, "entry")?);
        let instance = DialogueInstanceId::new(Self::allocate_identity(
            &mut self.next_dialogue_instance_id,
            "occurrence",
        )?);
        let entry = DialogueEntryState {
            id: entry_id,
            instance,
            stage: DialogueStageIndex::default(),
            waiting_for_advance: false,
            frame: frame.clone(),
        };
        let presentation = self
            .presentations
            .get_mut(&presentation_id)
            .expect("View definition index was created with its dialogue presentation");
        if replace {
            presentation.entries.clear();
        }
        presentation.entries.push(entry);
        presentation.active = Some(entry_id);
        presentation.bump_revision()
    }

    fn clear_view(
        &mut self,
        view: &DialogueViewDefinition,
    ) -> Result<(), DialoguePresentationStoreError> {
        let presentation_id = self.ensure_presentation(view)?;
        let presentation = self
            .presentations
            .get_mut(&presentation_id)
            .expect("View definition index was created with its dialogue presentation");
        presentation.entries.clear();
        presentation.active = None;
        presentation.bump_revision()
    }

    fn ensure_presentation(
        &mut self,
        view: &DialogueViewDefinition,
    ) -> Result<DialoguePresentationId, DialoguePresentationStoreError> {
        if let Some(id) = self.definitions.get(view) {
            return Ok(*id);
        }
        let id = DialoguePresentationId::new(Self::allocate_identity(
            &mut self.next_presentation_id,
            "presentation",
        )?);
        let presentation = DialoguePresentation {
            id,
            view: view.clone(),
            revision: DialogueRevision::default(),
            entries: Vec::new(),
            active: None,
        };
        self.definitions.insert(view.clone(), id);
        self.presentations.insert(id, presentation);
        Ok(id)
    }

    fn advance_target(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> Result<DialogueAdvanceOutcome, DialogueAdvanceRejection> {
        if self.presentations.is_empty() {
            return Err(DialogueAdvanceRejection::NoDialogue);
        }
        let dialogue = self
            .presentations
            .get_mut(&target.dialogue)
            .ok_or(DialogueAdvanceRejection::UnknownPresentation)?;
        if target.revision != dialogue.revision {
            return Err(DialogueAdvanceRejection::StaleRevision);
        }
        if dialogue.revision.next().is_none() {
            return Err(DialogueAdvanceRejection::RevisionExhausted);
        }
        if dialogue.active != Some(target.entry) {
            return Err(DialogueAdvanceRejection::StaleEntry);
        }
        let entry = dialogue
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
            dialogue
                .bump_revision()
                .map_err(|_| DialogueAdvanceRejection::RevisionExhausted)?;
            return Ok(DialogueAdvanceOutcome::Stage {
                revision: dialogue.revision,
                from,
                to,
                page,
                advance,
            });
        }
        let line = entry.frame.line.clone();
        entry.waiting_for_advance = false;
        dialogue
            .bump_revision()
            .map_err(|_| DialogueAdvanceRejection::RevisionExhausted)?;
        Ok(DialogueAdvanceOutcome::Line {
            revision: dialogue.revision,
            line,
        })
    }

    fn allocate_identity(
        cursor: &mut u64,
        identity: &'static str,
    ) -> Result<u64, DialoguePresentationStoreError> {
        let allocated = *cursor;
        *cursor = cursor
            .checked_add(1)
            .ok_or(DialoguePresentationStoreError::IdentityExhausted { identity })?;
        Ok(allocated)
    }

    fn validate_cursor(
        identity: &'static str,
        cursor: u64,
        greatest_live: Option<u64>,
    ) -> Result<(), DialoguePresentationStoreError> {
        if greatest_live.is_some_and(|greatest| cursor <= greatest) {
            return Err(DialoguePresentationStoreError::InvalidSnapshot {
                message: format!(
                    "dialogue {identity} cursor {cursor} is not newer than live identity {}",
                    greatest_live.expect("checked as present")
                ),
            });
        }
        Ok(())
    }
}
