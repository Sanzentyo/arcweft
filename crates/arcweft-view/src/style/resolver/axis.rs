//! Logical-axis resolution and canonical physical-property lowering.

use super::super::{
    ComputedViewAxes, ComputedViewTransition, ViewAxisUsageSet, ViewBoxAxisMode,
    ViewBoxAxisRevision, ViewInheritedBoxAxes, ViewPropertyKind, ViewPropertyValueTransform,
    ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleContribution, ViewStyleContributionSource,
    ViewStyleNodeKey, ViewStylePriority, ViewStyleSourceId,
};
use super::ViewStyleResolveError;

#[derive(Clone, Debug)]
pub(super) struct PendingViewStyleContribution {
    pub(super) property: ViewPropertyKind,
    pub(super) value: ViewSpecifiedValue,
    pub(super) operation: ViewStyleAssignOp,
    pub(super) priority: ViewStylePriority,
    pub(super) source: ViewStyleContributionSource,
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedViewAxes {
    pub(super) axes: ComputedViewAxes,
    pub(super) local_barrier: bool,
}

pub(super) fn resolve_axes(
    node: &ViewStyleNodeKey,
    inherited: ViewInheritedBoxAxes,
    contributions: &[PendingViewStyleContribution],
) -> ResolvedViewAxes {
    let inherited = ComputedViewAxes::from_inherited_seed(inherited);
    let Some(winner) = contributions
        .iter()
        .rev()
        .find(|contribution| contribution.property.is_axis_context())
    else {
        return ResolvedViewAxes {
            axes: inherited,
            local_barrier: false,
        };
    };
    let ViewSpecifiedValue::BoxAxes { value: mode } = winner.value else {
        return ResolvedViewAxes {
            axes: inherited,
            local_barrier: false,
        };
    };
    ResolvedViewAxes {
        axes: ComputedViewAxes::styled(
            mode,
            ViewBoxAxisRevision::for_local_provider(node, mode, winner.priority, &winner.source),
            winner.priority,
            winner.source.clone(),
        ),
        local_barrier: true,
    }
}

pub(super) fn resolve_contribution(
    contribution: PendingViewStyleContribution,
    mode: ViewBoxAxisMode,
) -> Result<Vec<ViewStyleContribution>, ViewStyleResolveError> {
    let PendingViewStyleContribution {
        property: authored_property,
        value,
        operation,
        priority,
        source,
    } = contribution;
    let expanded = authored_property.expanded_properties();
    let mut resolved = Vec::with_capacity(expanded.len().max(1));
    if expanded.is_empty() {
        resolved.push(resolve_longhand(
            authored_property,
            authored_property,
            &value,
            operation,
            priority,
            &source,
            mode,
        )?);
    } else {
        for expanded_property in expanded {
            resolved.push(resolve_longhand(
                authored_property,
                *expanded_property,
                &value,
                operation,
                priority,
                &source,
                mode,
            )?);
        }
    }
    Ok(resolved)
}

fn resolve_longhand(
    authored_property: ViewPropertyKind,
    expanded_property: ViewPropertyKind,
    value: &ViewSpecifiedValue,
    operation: ViewStyleAssignOp,
    priority: ViewStylePriority,
    source: &ViewStyleContributionSource,
    mode: ViewBoxAxisMode,
) -> Result<ViewStyleContribution, ViewStyleResolveError> {
    let resolution = expanded_property.resolve_for_axes(mode);
    let value = match (resolution.value_transform(), value) {
        (ViewPropertyValueTransform::SignedLength(sign), ViewSpecifiedValue::Length { value }) => {
            ViewSpecifiedValue::Length {
                value: value.checked_apply_axis_sign(sign).map_err(|_| {
                    ViewStyleResolveError::AxisValueOverflow {
                        style_source: contribution_source_id(source),
                        authored_property,
                        resolved_property: resolution.resolved(),
                        mode,
                    }
                })?,
            }
        }
        _ => value.clone(),
    };
    Ok(ViewStyleContribution::resolved(
        authored_property,
        expanded_property,
        resolution.resolved(),
        value,
        operation,
        priority,
        source.clone(),
    ))
}

pub(super) fn resolve_transitions(
    value: Option<&ViewSpecifiedValue>,
    mode: ViewBoxAxisMode,
) -> (Vec<ComputedViewTransition>, ViewAxisUsageSet) {
    let Some(ViewSpecifiedValue::Transition { value }) = value else {
        return (Vec::new(), ViewAxisUsageSet::NONE);
    };
    value.iter().fold(
        (Vec::with_capacity(value.len()), ViewAxisUsageSet::NONE),
        |(mut transitions, usage), transition| {
            let authored = transition.property();
            let resolution = authored.resolve_for_axes(mode);
            let usage = if authored.is_axis_dependent() {
                usage
                    .union(authored.axis_usage())
                    .union(ViewAxisUsageSet::TRANSITION_TARGET)
            } else {
                usage
            };
            transitions.push(ComputedViewTransition::new(
                authored,
                resolution.resolved(),
                mode,
                transition.duration_millis(),
                transition.delay_millis(),
            ));
            (transitions, usage)
        },
    )
}

fn contribution_source_id(source: &ViewStyleContributionSource) -> ViewStyleSourceId {
    match source {
        ViewStyleContributionSource::Inherited => ViewStyleSourceId::new(0),
        ViewStyleContributionSource::Sheet { declaration, .. }
        | ViewStyleContributionSource::Patch { declaration, .. } => *declaration,
    }
}
