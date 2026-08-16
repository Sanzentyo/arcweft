use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::runtime_id::{
    RuntimeDialogueContentPlanId, RuntimeDialogueMarkId, RuntimeDialogueValueSlotId,
    RuntimeFunctionSiteId, RuntimeLineTaskGroupId,
};

use super::RuntimeLineId;

/// Semantic role of one evaluated dialogue template value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeDialogueValueRole {
    Interpolation,
    Condition,
}

/// One plan-owned function site supplying a document-local dialogue slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueValueSite {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    function: RuntimeFunctionSiteId,
}

impl RuntimeDialogueValueSite {
    pub(crate) const fn new(
        slot: RuntimeDialogueValueSlotId,
        role: RuntimeDialogueValueRole,
        function: RuntimeFunctionSiteId,
    ) -> Self {
        Self {
            slot,
            role,
            function,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    #[must_use]
    pub const fn role(&self) -> RuntimeDialogueValueRole {
        self.role
    }

    #[must_use]
    pub const fn function(&self) -> RuntimeFunctionSiteId {
        self.function
    }
}

/// Exact execution mapping for one source-owned dialogue document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueContentPlan {
    line: RuntimeLineId,
    values: Box<[RuntimeDialogueValueSite]>,
    marks: Box<[RuntimeDialogueMark]>,
    line_task_group: Option<RuntimeLineTaskGroupId>,
}

/// One source-owned dialogue mark with its content-local typed identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueMark {
    id: RuntimeDialogueMarkId,
    label: String,
}

impl RuntimeDialogueMark {
    pub(crate) const fn new(id: RuntimeDialogueMarkId, label: String) -> Self {
        Self { id, label }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueMarkId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl RuntimeDialogueContentPlan {
    pub(crate) fn new(
        line: RuntimeLineId,
        values: Box<[RuntimeDialogueValueSite]>,
        marks: Box<[RuntimeDialogueMark]>,
    ) -> Self {
        Self {
            line,
            values,
            marks,
            line_task_group: None,
        }
    }

    #[must_use]
    pub const fn line(&self) -> &RuntimeLineId {
        &self.line
    }

    #[must_use]
    pub const fn values(&self) -> &[RuntimeDialogueValueSite] {
        &self.values
    }

    #[must_use]
    pub const fn marks(&self) -> &[RuntimeDialogueMark] {
        &self.marks
    }

    #[must_use]
    pub const fn line_task_group(&self) -> Option<RuntimeLineTaskGroupId> {
        self.line_task_group
    }

    #[must_use]
    pub fn resolve_mark_label(&self, label: &str) -> Option<RuntimeDialogueMarkId> {
        self.marks
            .iter()
            .find(|mark| mark.label() == label)
            .map(RuntimeDialogueMark::id)
    }

    pub(crate) fn attach_line_task_group(&mut self, group: RuntimeLineTaskGroupId) -> bool {
        if self.line_task_group.is_some() {
            return false;
        }
        self.line_task_group = Some(group);
        true
    }
}

/// Immutable plan-owned dialogue execution table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDialogueContentPlanTable {
    rows: Box<[RuntimeDialogueContentPlan]>,
    by_line: BTreeMap<RuntimeLineId, RuntimeDialogueContentPlanId>,
}

impl RuntimeDialogueContentPlanTable {
    #[must_use]
    pub fn get(&self, id: RuntimeDialogueContentPlanId) -> Option<&RuntimeDialogueContentPlan> {
        self.rows.get(id.get().get().checked_sub(1)? as usize)
    }

    #[must_use]
    pub fn find_line(&self, line: &RuntimeLineId) -> Option<RuntimeDialogueContentPlanId> {
        self.by_line.get(line).copied()
    }

    #[must_use]
    pub const fn rows(&self) -> &[RuntimeDialogueContentPlan] {
        &self.rows
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeDialogueContentPlanTableBuilder {
    rows: Vec<RuntimeDialogueContentPlan>,
    by_line: BTreeMap<RuntimeLineId, RuntimeDialogueContentPlanId>,
}

impl RuntimeDialogueContentPlanTableBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(
        &mut self,
        plan: RuntimeDialogueContentPlan,
    ) -> Result<RuntimeDialogueContentPlanId, RuntimeDialogueContentPlanTableError> {
        if self.by_line.contains_key(plan.line()) {
            return Err(RuntimeDialogueContentPlanTableError::DuplicateLine {
                line: plan.line().clone(),
            });
        }
        let ordinal = u32::try_from(self.rows.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeDialogueContentPlanTableError::TooManyRows)?;
        let id = RuntimeDialogueContentPlanId::from_accepted_ordinal(ordinal);
        self.by_line.insert(plan.line().clone(), id);
        self.rows.push(plan);
        Ok(id)
    }

    pub(crate) fn get(
        &self,
        id: RuntimeDialogueContentPlanId,
    ) -> Option<&RuntimeDialogueContentPlan> {
        self.rows.get(id.get().get().checked_sub(1)? as usize)
    }

    pub(crate) fn get_mut(
        &mut self,
        id: RuntimeDialogueContentPlanId,
    ) -> Option<&mut RuntimeDialogueContentPlan> {
        self.rows.get_mut(id.get().get().checked_sub(1)? as usize)
    }

    pub(crate) fn finish(self) -> RuntimeDialogueContentPlanTable {
        RuntimeDialogueContentPlanTable {
            rows: self.rows.into_boxed_slice(),
            by_line: self.by_line,
        }
    }
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RuntimeDialogueContentPlanTableError {
    #[error("runtime dialogue content plan repeats line {line}")]
    DuplicateLine { line: RuntimeLineId },
    #[error("runtime dialogue content plan table exceeds its u32 row limit")]
    TooManyRows,
}
