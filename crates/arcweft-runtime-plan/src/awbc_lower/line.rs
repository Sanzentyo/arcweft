use crate::awbc_lower::inventory::AwbcInventory;
use arcweft_core::awbc::schema::{AwbcContentUnitId, AwbcLineTaskGroupId};
use arcweft_core::plan::RuntimeLineId;

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
        line: &RuntimeLineId,
        line_task_group: AwbcLineTaskGroupId,
    ) -> AwbcContentUnitId {
        let line = line.public_label().into_string();
        let id = self
            .inventory
            .intern_content_unit(line.as_str(), Some(line_task_group));
        if let Some(unit) = self.inventory.program.content_units.get_mut(id.index()) {
            unit.line_task_group = Some(line_task_group);
        }
        id
    }
}
