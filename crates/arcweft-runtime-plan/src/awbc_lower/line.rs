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
    ) -> Result<AwbcContentUnitId, crate::awbc_lower::inventory::AwbcLowerDiagnostic> {
        let line = content.line().public_label().into_string();
        let Some(_template) = self
            .inventory
            .dialogue_template_manifest(content.template())
            .cloned()
        else {
            return Err(crate::awbc_lower::inventory::AwbcLowerDiagnostic::error(
                format!("dialogue.line.{line}"),
                format!(
                    "dialogue plan references template {} absent from the text-model catalog",
                    content.template()
                ),
            ));
        };
        if let Some(group) = line_task_group
            && self
                .inventory
                .program
                .line_task_groups
                .get(group.index())
                .is_none()
        {
            return Err(crate::awbc_lower::inventory::AwbcLowerDiagnostic::error(
                format!("dialogue.line.{line}"),
                "dialogue content references a missing line-task group",
            ));
        }
        let id = self.inventory.intern_content_unit(
            line.as_str(),
            content.template(),
            line_task_group,
        )?;
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
        Ok(id)
    }
}
