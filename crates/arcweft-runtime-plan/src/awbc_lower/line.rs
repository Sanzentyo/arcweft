use crate::awbc_lower::inventory::AwbcInventory;
use arcweft_core::awbc::schema::{AwbcContentUnitId, AwbcDialogueMark, AwbcLineTaskGroupId};
use arcweft_core::plan::RuntimeDialogueContentPlan;

/// Dialogue/content side of AWBC lowering.
pub struct AwbcLineLowerer<'a> {
    inventory: &'a mut AwbcInventory,
}

impl<'a> AwbcLineLowerer<'a> {
    pub fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self { inventory }
    }

    pub fn content_for_line(
        &mut self,
        content: &RuntimeDialogueContentPlan,
        line_task_group: Option<AwbcLineTaskGroupId>,
    ) -> AwbcContentUnitId {
        let line = content.line().public_label().into_string();
        let id = self
            .inventory
            .intern_content_unit(line.as_str(), line_task_group);
        let marks = content
            .marks()
            .iter()
            .map(|mark| AwbcDialogueMark {
                id: mark.id(),
                label: self.inventory.intern_string(mark.label()),
            })
            .collect();
        if let Some(unit) = self.inventory.program.content_units.get_mut(id.index()) {
            unit.line_task_group = line_task_group;
            unit.marks = marks;
            unit.effect_site_count = content.effect_site_count().get();
        }
        id
    }
}
