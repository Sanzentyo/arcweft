//! Validates typed text and action-control bindings on cold restore.

use super::{
    BundleViewActionButtonOutput, BundleViewMountOutput, BundleViewPaintItem,
    BundleViewRuntimeError, BundleViewStyleNodeId, BundleViewStyleNodeKind, BundleViewTextOrigin,
};
use arcweft_bundle::resource_codec::view::{
    ViewInstructionSpan, ViewProgramInstruction, ViewProgramResource,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate(
    mount: &BundleViewMountOutput,
    span: ViewInstructionSpan,
    program: &ViewProgramResource,
) -> Result<(), BundleViewRuntimeError> {
    let failure = |message| BundleViewRuntimeError::PresentationFrameMismatch {
        message: format!("retained publication on mount {:?}: {message}", mount.mount),
    };
    let instruction = |id: &BundleViewStyleNodeId| {
        let belongs_to_mount =
            id.path.segments().starts_with(mount.path.segments()) && id.path.validate().is_ok();
        let belongs_to_definition =
            (span.start_instruction..span.end_instruction).contains(&id.instruction);
        if belongs_to_mount && belongs_to_definition {
            program.instructions.get(id.instruction as usize)
        } else {
            None
        }
    };
    let mut claimed_blocks = BTreeSet::new();
    let mut block_nodes = BTreeSet::new();
    let mut node_order = Vec::new();
    for node in &mount.style_nodes {
        let index = match node.kind {
            BundleViewStyleNodeKind::Text { output }
            | BundleViewStyleNodeKind::RichText { output } => output,
            _ => continue,
        };
        let row = mount
            .text_output(index)
            .ok_or_else(|| failure("unknown text output"))?;
        if row.block_for(node).is_none()
            || row.value.style_target_kind() != node.kind.style_target_kind()
            || !claimed_blocks.insert(index.index())
            || !block_nodes.insert((&node.path, node.instruction))
        {
            return Err(failure("text output belongs to a different retained node"));
        }
        node_order.push(index);
    }
    let mut labels = BTreeSet::new();
    for (index, row) in mount.text.iter().enumerate() {
        match &row.origin {
            BundleViewTextOrigin::Block { node, target } => {
                let Some(ViewProgramInstruction::EmitText {
                    text_source,
                    text_block,
                    ..
                }) = instruction(node)
                else {
                    return Err(failure("text block differs from its accepted instruction"));
                };
                let Some(block) = program
                    .text_blocks
                    .iter()
                    .find(|block| block.public_id == *text_block)
                else {
                    return Err(failure("text block is missing from the accepted View"));
                };
                if row.source_id != *text_source
                    || target.public_id != node.path.qualify_repeated_target(&block.public_id)
                    || target.bounds != block.bounds
                    || target.selection_policy != block.selection_policy
                    || target.containing_scroll_region != block.containing_scroll_region
                    || !claimed_blocks.contains(&index)
                {
                    return Err(failure("text block differs from its accepted instruction"));
                }
            }
            BundleViewTextOrigin::SemanticLabel { instruction: id } => {
                if !matches!(
                    instruction(id),
                    Some(ViewProgramInstruction::AttachSemantic {
                        label_text_source: Some(source), ..
                    }) if source == &row.source_id
                ) || !labels.insert((&id.path, id.instruction))
                {
                    return Err(failure(
                        "semantic text differs from its accepted instruction",
                    ));
                }
            }
        }
    }
    let paint_order = mount.paint.iter().filter_map(|paint| match paint {
        BundleViewPaintItem::Text { output } => Some(*output),
        _ => None,
    });
    if !node_order.into_iter().eq(paint_order) {
        return Err(failure("text paint order differs from retained node order"));
    }
    validate_action_buttons(mount, program, instruction).map_err(failure)
}

fn validate_action_buttons<'a>(
    mount: &BundleViewMountOutput,
    program: &'a ViewProgramResource,
    instruction: impl Fn(&BundleViewStyleNodeId) -> Option<&'a ViewProgramInstruction>,
) -> Result<(), &'static str> {
    let mut nodes = BTreeMap::new();
    for node in &mount.style_nodes {
        if let BundleViewStyleNodeKind::Element {
            element,
            target: Some(target),
        } = &node.kind
            && element.is_action_control()
            && nodes
                .insert((&node.path, node.instruction), (element, target))
                .is_some()
        {
            return Err("action button repeats its retained node");
        }
    }
    for output in &mount.action_buttons {
        let Some((element, target)) = nodes.remove(&(&output.node.path, output.node.instruction))
        else {
            return Err("action button has no retained node");
        };
        let Some(ViewProgramInstruction::OpenElement {
            element: declared_element,
            target: Some(declaration),
            ..
        }) = instruction(&output.node)
        else {
            return Err("action button differs from its accepted instruction");
        };
        let Some(button) = program
            .action_buttons
            .iter()
            .find(|button| button.public_id == *declaration)
        else {
            return Err("action button is missing from the accepted View");
        };
        let expected = BundleViewActionButtonOutput::new(
            output.node.clone(),
            button.runtime_value(output.value.label.clone(), output.value.enabled),
            target,
        );
        if element != declared_element
            || *target != output.node.path.qualify_repeated_target(declaration)
            || *output != expected
            || (!output.value.enabled && mount.activation_binding(&output.node).is_some())
        {
            return Err("action button differs from its accepted instruction");
        }
    }
    if !nodes.is_empty() {
        return Err("action button is missing for its retained node");
    }
    Ok(())
}
