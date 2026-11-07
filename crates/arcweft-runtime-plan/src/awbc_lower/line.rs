use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{AwbcContentUnitId, AwbcDialogueMark, AwbcLineTaskGroupId};
use arcweft_core::plan::{RuntimeDialogueContentPlan, RuntimePlan};
use arcweft_core::value::RuntimeLocalRead;

/// Dialogue/content side of AWBC lowering.
pub struct AwbcLineLowerer<'a> {
    inventory: &'a mut AwbcInventory,
}

impl<'a> AwbcLineLowerer<'a> {
    pub fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self { inventory }
    }

    pub fn content_for_line<'plan>(
        &mut self,
        content: &RuntimeDialogueContentPlan,
        plan: &'plan RuntimePlan,
    ) -> Result<(AwbcContentUnitId, &'plan [RuntimeLocalRead]), AwbcLowerDiagnostic> {
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
        // The admitted plan owns both the group ordinal and its captures.
        // A reference can be emitted before that group's AWBC body is lowered.
        let (line_task_group, captures) = match content.line_task_group() {
            Some(group) => {
                let row = plan.line_task_groups().get(group.index()).ok_or_else(|| {
                    AwbcLowerDiagnostic::error(
                        format!("dialogue.line.{line}"),
                        "dialogue content references a missing RuntimePlan line-task group",
                    )
                })?;
                (
                    Some(AwbcLineTaskGroupId(table_index(group.index()))),
                    row.initial_inputs(),
                )
            }
            None => (None, &[][..]),
        };
        let id = self
            .inventory
            .intern_content_unit(&content.key(), line_task_group)?;
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
        Ok((id, captures))
    }
}
