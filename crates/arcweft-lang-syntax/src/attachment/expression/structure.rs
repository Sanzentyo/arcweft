//! Structural extraction and projection validation for attached expressions.

use super::{
    AstNode, AttachedCallTypeChild, AttachedClosureParameter, AttachedExpressionChild,
    AttachedPath, AttachedPatternNode, AttachedTypeRefNode, BlockKind, ExpressionComponentRole,
    ExpressionFamily, ExpressionProjection, ExpressionRecordFieldPart, FamilyNode, FamilySpec,
    PathKind, PatternFamily, RecoveryFamily, SyntaxAccessError, SyntaxCallArgumentPart,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeApplicationComponentRole,
    SyntaxCallTypeArgumentPart, SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole,
    SyntaxClosureParameterPart, SyntaxKind, SyntaxNameIssue, SyntaxNodeHandle, SyntaxRecordField,
    SyntaxRole,
};

pub(super) fn attached_path(syntax: &SyntaxNodeHandle) -> Result<AttachedPath, SyntaxAccessError> {
    let children = syntax.children_with_role(SyntaxRole::Target);
    let [path] = children.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    if path.kind() != SyntaxKind::Path {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    AttachedPath::from_syntax(path.cast::<PathKind>()?)
}

pub(super) fn attached_path_projection(
    syntax: &SyntaxNodeHandle,
) -> Result<(Option<AttachedPath>, Option<AttachedTypeRefNode>), SyntaxAccessError> {
    let paths = syntax.children_with_role(SyntaxRole::Target);
    let types = syntax.children_with_role(SyntaxRole::Type);
    match (paths.as_slice(), types.as_slice()) {
        ([path], []) if path.kind() == SyntaxKind::Path => Ok((
            Some(AttachedPath::from_syntax(path.cast::<PathKind>()?)?),
            None,
        )),
        ([], [type_ref]) => {
            let type_ref = AttachedTypeRefNode::from_syntax(type_ref.clone())?;
            if type_ref.value().nominal_path().is_none() {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            Ok((None, Some(type_ref)))
        }
        _ => Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
    }
}

pub(super) fn attached_block(
    syntax: &SyntaxNodeHandle,
) -> Result<AstNode<BlockKind>, SyntaxAccessError> {
    let children = syntax.children_with_role(SyntaxRole::Body);
    let [block] = children.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    if block.kind() != SyntaxKind::Block {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    block.cast::<BlockKind>().map_err(SyntaxAccessError::from)
}

pub(super) fn attached_pattern(
    syntax: &SyntaxNodeHandle,
) -> Result<AttachedPatternNode, SyntaxAccessError> {
    let children = syntax.children_with_role(SyntaxRole::Pattern);
    let [pattern] = children.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    FamilyNode::<PatternFamily>::new(pattern.clone())?.semantic()
}

pub(super) fn attached_composite_children(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedExpressionChild]>, SyntaxAccessError> {
    if matches!(projection, ExpressionProjection::Error) {
        // A standalone E35 recovery has no child. When generic recovery wraps
        // a successfully parsed prefix, retain that exact central expression
        // identity so its independently owned final HIR record is not lost.
        // The outer Error payload still owns no semantic child reference.
        let prefixes = syntax.children_with_role(SyntaxRole::Operand);
        return match prefixes.as_slice() {
            [] => Ok(Box::new([])),
            [prefix]
                if ExpressionFamily::accepts(prefix.kind())
                    && prefix.kind() != SyntaxKind::MissingExpression =>
            {
                Ok(Box::new([AttachedExpressionChild::Authored {
                    ordinal: 0,
                    expression: FamilyNode::<ExpressionFamily>::new(prefix.clone())?,
                    source: prefix.source_span(),
                }]))
            }
            _ => Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
        };
    }
    if matches!(projection, ExpressionProjection::Choice) {
        // Choice owns its public ID and all candidate/plan expressions through
        // the specialized attached Choice relation. They are not generic
        // positional expression children of the Choice payload.
        return Ok(Box::new([]));
    }
    if let ExpressionProjection::Call(call) = projection {
        return attached_call_children(syntax, call, components);
    }
    if let ExpressionProjection::Index(index) = projection {
        return attached_postfix_index_children(syntax, index, components);
    }
    if let ExpressionProjection::DialogueContentApplication(application) = projection {
        return attached_dialogue_application_children(syntax, application, components);
    }
    if let ExpressionProjection::Record(fields) | ExpressionProjection::RecordLiteral(fields) =
        projection
    {
        return attached_record_children(syntax, fields, components);
    }
    let child_nodes = syntax
        .children()
        .into_iter()
        .filter(|child| ExpressionFamily::accepts(child.kind()))
        .collect::<Vec<_>>();
    let slots = match projection {
        ExpressionProjection::Tuple(slots) | ExpressionProjection::BracketSequence(slots) => slots
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, slot)| {
                u32::try_from(ordinal)
                    .map(|ordinal| (ordinal, slot))
                    .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })
            })
            .collect::<Result<Vec<_>, _>>()?,
        ExpressionProjection::ArrayRepeat(slots) | ExpressionProjection::Pipe(slots) => slots
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, slot)| {
                u32::try_from(ordinal)
                    .map(|ordinal| (ordinal, slot))
                    .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })
            })
            .collect::<Result<Vec<_>, _>>()?,
        ExpressionProjection::Index(index) => {
            vec![(0, index.target()), (1, index.index())]
        }
        ExpressionProjection::DialogueContentApplication(_) => {
            vec![(0, crate::expressions::SyntaxExpressionSlot::Authored)]
        }
        ExpressionProjection::PostfixBracket(_) => {
            vec![(0, crate::expressions::SyntaxExpressionSlot::Authored)]
        }
        ExpressionProjection::Select(_) => {
            vec![(0, crate::expressions::SyntaxExpressionSlot::Authored)]
        }
        ExpressionProjection::Try { operand, .. } | ExpressionProjection::Await { operand, .. } => {
            vec![(0, *operand)]
        }
        ExpressionProjection::Borrow { operand, .. }
        | ExpressionProjection::Dereference { operand }
        | ExpressionProjection::Unary { operand, .. } => vec![(0, *operand)],
        ExpressionProjection::Binary { left, right, .. } => vec![(0, *left), (1, *right)],
        ExpressionProjection::Closure(closure) => vec![(0, closure.body())],
        ExpressionProjection::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let mut slots = vec![(0, *condition), (1, *then_branch)];
            if let Some(else_branch) = else_branch {
                slots.push((2, *else_branch));
            }
            slots
        }
        ExpressionProjection::IfLet {
            scrutinee,
            guard,
            then_branch,
            else_branch,
        } => {
            let mut slots = vec![(0, *scrutinee)];
            if let Some(guard) = guard {
                slots.push((1, *guard));
            }
            slots.push((2, *then_branch));
            if let Some(else_branch) = else_branch {
                slots.push((3, *else_branch));
            }
            slots
        }
        ExpressionProjection::Match(projection) => vec![(0, projection.scrutinee())],
        ExpressionProjection::Range { start, end, .. } => {
            let mut slots =
                Vec::with_capacity(usize::from(start.is_some()) + usize::from(end.is_some()));
            if let Some(start) = start {
                slots.push((0, *start));
            }
            if let Some(end) = end {
                slots.push((1, *end));
            }
            slots
        }
        ExpressionProjection::Unit
        | ExpressionProjection::NumericBracketSequence(_)
        | ExpressionProjection::Block
        | ExpressionProjection::ComputationBlock(_)
        | ExpressionProjection::NamedBlock(_)
        | ExpressionProjection::Thread(_)
        | ExpressionProjection::Choice => Vec::new(),
        _ if child_nodes
            .iter()
            .any(|child| matches!(child.role(), SyntaxRole::Element(_))) =>
        {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        _ => return Ok(Box::new([])),
    };
    if child_nodes.len() != slots.len() {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }

    child_nodes
        .into_iter()
        .zip(slots)
        .map(|(child, (ordinal, slot))| {
            let expected_role = semantic_child_role(projection, ordinal);
            let role_matches = expected_role.is_some_and(|role| child.role() == role)
                || child.role()
                    == SyntaxRole::Bucket(u16::try_from(ordinal).map_err(|_| {
                        SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }
                    })?)
                    && matches!(projection, ExpressionProjection::BracketSequence(_))
                    && syntax
                        .parent()
                        .is_some_and(|parent| parent.kind() == SyntaxKind::MetricBucketsMember);
            if !role_matches
                || component_range_for_slot(projection, components, ordinal).is_none_or(
                    |component| !component_matches_semantic_child(syntax, &child, component),
                )
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            match slot {
                crate::expressions::SyntaxExpressionSlot::Authored
                    if child.kind() != SyntaxKind::MissingExpression =>
                {
                    Ok(AttachedExpressionChild::Authored {
                        ordinal,
                        expression: FamilyNode::<ExpressionFamily>::new(child)?,
                        source: syntax.source_span_for_range(
                            component_range_for_slot(projection, components, ordinal)
                                .expect("validated composite slot retains one source component"),
                        ),
                    })
                }
                crate::expressions::SyntaxExpressionSlot::Missing
                    if child.kind() == SyntaxKind::MissingExpression =>
                {
                    Ok(AttachedExpressionChild::Missing {
                        ordinal,
                        recovery: FamilyNode::<RecoveryFamily>::new(child)?,
                    })
                }
                _ => Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn attached_postfix_index_children(
    syntax: &SyntaxNodeHandle,
    index: &crate::expressions::SyntaxIndexProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedExpressionChild]>, SyntaxAccessError> {
    let targets = syntax.children_with_role(SyntaxRole::Target);
    let payloads = syntax.children_with_role(SyntaxRole::Payload);
    let ([target], [payload]) = (targets.as_slice(), payloads.as_slice()) else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let mut payload_expressions = Vec::new();
    collect_expression_roots(payload, &mut payload_expressions);
    let [index_node] = payload_expressions.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let target_range = components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::Target)
        .map(|component| component.range())
        .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
    let index_range = components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::Index)
        .map(|component| component.range())
        .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
    if !ExpressionFamily::accepts(target.kind())
        || !component_matches_semantic_child(syntax, target, target_range)
        || !component_matches_semantic_child(syntax, index_node, index_range)
    {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }

    let target = match index.target() {
        crate::expressions::SyntaxExpressionSlot::Authored
            if target.kind() != SyntaxKind::MissingExpression =>
        {
            AttachedExpressionChild::Authored {
                ordinal: 0,
                expression: FamilyNode::<ExpressionFamily>::new(target.clone())?,
                source: syntax.source_span_for_range(target_range),
            }
        }
        crate::expressions::SyntaxExpressionSlot::Missing
            if target.kind() == SyntaxKind::MissingExpression =>
        {
            AttachedExpressionChild::Missing {
                ordinal: 0,
                recovery: FamilyNode::<RecoveryFamily>::new(target.clone())?,
            }
        }
        _ => return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
    };
    let index = match index.index() {
        crate::expressions::SyntaxExpressionSlot::Authored
            if index_node.kind() != SyntaxKind::MissingExpression =>
        {
            AttachedExpressionChild::Authored {
                ordinal: 1,
                expression: FamilyNode::<ExpressionFamily>::new(index_node.clone())?,
                source: syntax.source_span_for_range(index_range),
            }
        }
        crate::expressions::SyntaxExpressionSlot::Missing
            if index_node.kind() == SyntaxKind::MissingExpression =>
        {
            AttachedExpressionChild::Missing {
                ordinal: 1,
                recovery: FamilyNode::<RecoveryFamily>::new(index_node.clone())?,
            }
        }
        _ => return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
    };
    Ok(Box::new([target, index]))
}

fn collect_expression_roots(node: &SyntaxNodeHandle, expressions: &mut Vec<SyntaxNodeHandle>) {
    for child in node.children() {
        if ExpressionFamily::accepts(child.kind()) {
            expressions.push(child);
        } else {
            collect_expression_roots(&child, expressions);
        }
    }
}

fn attached_dialogue_application_children(
    syntax: &SyntaxNodeHandle,
    application: &crate::expressions::SyntaxDialogueApplicationProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedExpressionChild]>, SyntaxAccessError> {
    let targets = syntax.children_with_role(SyntaxRole::Target);
    let payload_role = match application.form() {
        crate::expressions::SyntaxDialogueApplicationForm::Bracket { .. } => SyntaxRole::Payload,
        crate::expressions::SyntaxDialogueApplicationForm::Colon => SyntaxRole::Content,
    };
    let payloads = syntax.children_with_role(payload_role);
    let ([target], [payload]) = (targets.as_slice(), payloads.as_slice()) else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let target_range = components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::Target)
        .map(|component| component.range())
        .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
    if !ExpressionFamily::accepts(target.kind())
        || target.kind() == SyntaxKind::MissingExpression
        || !component_matches_semantic_child(syntax, target, target_range)
    {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    let mut children = vec![AttachedExpressionChild::Authored {
        ordinal: 0,
        expression: FamilyNode::<ExpressionFamily>::new(target.clone())?,
        source: syntax.source_span_for_range(target_range),
    }];

    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        application.content()
    else {
        let mut nested = Vec::new();
        collect_expression_roots(payload, &mut nested);
        return if nested.is_empty() {
            Ok(children.into_boxed_slice())
        } else {
            Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })
        };
    };

    let mut expected = Vec::new();
    for (ordinal, node) in content.nodes().iter().enumerate() {
        let crate::expressions::SyntaxDialogueNodeProjection::Interpolation(slot) = node else {
            continue;
        };
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        let range = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::DialogueNode {
                        ordinal,
                        part: crate::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
                    }
            })
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        expected.push((*slot, SyntaxRole::Operand, range));
    }
    for (tag, projection) in content.tags().iter().enumerate() {
        let (slot, role) = match projection.payload() {
            crate::expressions::SyntaxRichTextTagPayloadProjection::FxCall(slot)
            | crate::expressions::SyntaxRichTextTagPayloadProjection::DialogueCall(slot) => {
                (*slot, SyntaxRole::Operand)
            }
            crate::expressions::SyntaxRichTextTagPayloadProjection::Condition(slot) => {
                (*slot, SyntaxRole::Condition)
            }
            crate::expressions::SyntaxRichTextTagPayloadProjection::Arguments
            | crate::expressions::SyntaxRichTextTagPayloadProjection::None => continue,
        };
        let tag = u32::try_from(tag)
            .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        let range = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::RichTextTag {
                        tag,
                        part: crate::expressions::SyntaxRichTextTagSourcePart::Payload,
                    }
            })
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        expected.push((slot, role, range));
    }
    expected.sort_by_key(|(_, _, range)| (range.start(), range.end()));

    let mut nested = Vec::new();
    collect_expression_roots(payload, &mut nested);
    if nested.len() != expected.len() {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    for (position, (expression, (slot, role, owner_range))) in
        nested.into_iter().zip(expected).enumerate()
    {
        if expression.role() != role
            || expression.range().start() < owner_range.start()
            || expression.range().end() > owner_range.end()
        {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let ordinal = u32::try_from(
            position
                .checked_add(1)
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?,
        )
        .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        match slot {
            crate::expressions::SyntaxExpressionSlot::Authored
                if expression.kind() != SyntaxKind::MissingExpression =>
            {
                children.push(AttachedExpressionChild::Authored {
                    ordinal,
                    expression: FamilyNode::<ExpressionFamily>::new(expression.clone())?,
                    source: expression.source_span(),
                });
            }
            crate::expressions::SyntaxExpressionSlot::Missing
                if expression.kind() == SyntaxKind::MissingExpression =>
            {
                children.push(AttachedExpressionChild::Missing {
                    ordinal,
                    recovery: FamilyNode::<RecoveryFamily>::new(expression)?,
                });
            }
            _ => {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
        }
    }
    Ok(children.into_boxed_slice())
}

fn attached_call_children(
    syntax: &SyntaxNodeHandle,
    call: &SyntaxCallProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedExpressionChild]>, SyntaxAccessError> {
    if let SyntaxCallProjection::CallbackBlock(callback) = call {
        let callees = syntax.children_with_role(SyntaxRole::Callee);
        let callbacks = syntax.children_with_role(SyntaxRole::Argument(0));
        let ([callee], [callback_node]) = (callees.as_slice(), callbacks.as_slice()) else {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        };
        let callee_range = components
            .iter()
            .find(|component| component.role() == ExpressionComponentRole::CallCallee)
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        let callback_range = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Value,
                    }
            })
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        if !ExpressionFamily::accepts(callee.kind())
            || callee.kind() == SyntaxKind::MissingExpression
            || !component_matches_semantic_child(syntax, callee, callee_range)
            || !component_matches_semantic_child(syntax, callback_node, callback_range)
        {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let mut children = vec![AttachedExpressionChild::Authored {
            ordinal: 0,
            expression: FamilyNode::<ExpressionFamily>::new(callee.clone())?,
            source: syntax.source_span_for_range(callee_range),
        }];
        match callback.callback() {
            crate::expressions::SyntaxExpressionSlot::Authored
                if callback_node.kind() == SyntaxKind::ClosureExpression =>
            {
                children.push(AttachedExpressionChild::Authored {
                    ordinal: 1,
                    expression: FamilyNode::<ExpressionFamily>::new(callback_node.clone())?,
                    source: syntax.source_span_for_range(callback_range),
                });
            }
            crate::expressions::SyntaxExpressionSlot::Missing
                if callback_node.kind() == SyntaxKind::MissingExpression =>
            {
                children.push(AttachedExpressionChild::Missing {
                    ordinal: 1,
                    recovery: FamilyNode::<RecoveryFamily>::new(callback_node.clone())?,
                });
            }
            _ => {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
        }
        return Ok(children.into_boxed_slice());
    }
    let Some(call) = call.parenthesized() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let callees = syntax.children_with_role(SyntaxRole::Callee);
    let callee_component_role = match call.callee() {
        SyntaxCallCalleeProjection::Ordinary => Some(ExpressionComponentRole::CallCallee),
        SyntaxCallCalleeProjection::UnresolvedDot { .. } => {
            Some(ExpressionComponentRole::CallAssociatedReceiver)
        }
        SyntaxCallCalleeProjection::Associated { .. } => None,
    };
    let mut children = Vec::new();
    match (callee_component_role, callees.as_slice()) {
        (Some(component_role), [callee]) => {
            let callee_component = components
                .iter()
                .find(|component| component.role() == component_role)
                .map(|component| component.range())
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            if !ExpressionFamily::accepts(callee.kind())
                || callee.kind() == SyntaxKind::MissingExpression
                || !component_matches_semantic_child(syntax, callee, callee_component)
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            children.push(AttachedExpressionChild::Authored {
                ordinal: 0,
                expression: FamilyNode::<ExpressionFamily>::new(callee.clone())?,
                source: syntax.source_span_for_range(callee_component),
            });
        }
        (None, []) => {}
        _ => return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
    }

    for (argument, projection) in call.arguments().iter().enumerate() {
        let argument_ordinal = u16::try_from(argument)
            .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        // `ArgumentList` is a structural wrapper and therefore does not own an
        // attached identity. Its `CallArgument` descendants are attached
        // directly to the Call while retaining their source-order role.
        let argument_nodes = syntax.children_with_role(SyntaxRole::Argument(argument_ordinal));
        let [argument_node] = argument_nodes.as_slice() else {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        };
        if argument_node.kind() != SyntaxKind::CallArgument {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let values = argument_node.children_with_role(SyntaxRole::Operand);
        let [value] = values.as_slice() else {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        };
        let value_component = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::CallArgument {
                        argument: argument_ordinal,
                        part: SyntaxCallArgumentPart::Value,
                    }
            })
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        if !ExpressionFamily::accepts(value.kind())
            || !component_matches_semantic_child(syntax, value, value_component)
        {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let ordinal = u32::try_from(argument)
            .ok()
            .and_then(|argument| argument.checked_add(1))
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        match projection.value() {
            crate::expressions::SyntaxExpressionSlot::Authored
                if value.kind() != SyntaxKind::MissingExpression =>
            {
                children.push(AttachedExpressionChild::Authored {
                    ordinal,
                    expression: FamilyNode::<ExpressionFamily>::new(value.clone())?,
                    source: syntax.source_span_for_range(value_component),
                });
            }
            crate::expressions::SyntaxExpressionSlot::Missing
                if value.kind() == SyntaxKind::MissingExpression =>
            {
                children.push(AttachedExpressionChild::Missing {
                    ordinal,
                    recovery: FamilyNode::<RecoveryFamily>::new(value.clone())?,
                });
            }
            _ => {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
        }
    }
    Ok(children.into_boxed_slice())
}

pub(super) fn attached_call_type_children(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedCallTypeChild]>, SyntaxAccessError> {
    let ExpressionProjection::Call(call) = projection else {
        return Ok(Box::new([]));
    };
    let Some(call) = call.parenthesized() else {
        return Ok(Box::new([]));
    };
    let receiver_component = components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::CallAssociatedReceiver)
        .map(|component| component.range());
    let mut type_children = Vec::new();
    let direct_types = syntax.children_with_role(SyntaxRole::Type);
    let mut direct_type_cursor = 0_usize;
    match call.callee() {
        SyntaxCallCalleeProjection::Ordinary => {
            if receiver_component.is_some() {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
        }
        SyntaxCallCalleeProjection::UnresolvedDot { .. } => {
            let callees = syntax.children_with_role(SyntaxRole::Callee);
            let [callee] = callees.as_slice() else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            let types = callee.children_with_role(SyntaxRole::Type);
            let [type_ref] = types.as_slice() else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            let type_ref = AttachedTypeRefNode::from_syntax(type_ref.clone())?;
            let receiver_component = receiver_component
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            if type_ref.value().nominal_path().is_none()
                || type_ref.whole_source_span().range() != receiver_component
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            type_children.push(AttachedCallTypeChild {
                role: SyntaxCallTypeChildRole::DotNominalReceiver,
                node: type_ref,
            });
        }
        SyntaxCallCalleeProjection::Associated { .. } => {
            let Some(type_ref) = direct_types.get(direct_type_cursor) else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            direct_type_cursor += 1;
            let type_ref = AttachedTypeRefNode::from_syntax(type_ref.clone())?;
            let receiver_component = receiver_component
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            if type_ref.value().nominal_path().is_none()
                || type_ref.whole_source_span().range() != receiver_component
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            type_children.push(AttachedCallTypeChild {
                role: SyntaxCallTypeChildRole::AssociatedReceiver,
                node: type_ref,
            });
        }
    }

    if let Some(application) = call.explicit_type_application() {
        for (ordinal, projection) in application.arguments().iter().enumerate() {
            let ordinal = u16::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            if matches!(projection, SyntaxCallTypeArgumentProjection::Missing) {
                continue;
            }
            let Some(type_ref) = direct_types.get(direct_type_cursor) else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            direct_type_cursor += 1;
            let type_ref = AttachedTypeRefNode::from_syntax(type_ref.clone())?;
            let source = components
                .iter()
                .find(|component| {
                    component.role()
                        == ExpressionComponentRole::CallTypeApplication(
                            SyntaxCallTypeApplicationComponentRole::Argument {
                                argument: ordinal,
                                part: SyntaxCallTypeArgumentPart::Type,
                            },
                        )
                })
                .map(|component| component.range())
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            let recovery = matches!(type_ref.value(), crate::types::TypeRef::Recovery(_));
            if type_ref.whole_source_span().range() != source
                || recovery
                    != matches!(projection, SyntaxCallTypeArgumentProjection::InvalidPresent)
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            type_children.push(AttachedCallTypeChild {
                role: SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal },
                node: type_ref,
            });
        }
    }
    if direct_type_cursor != direct_types.len() {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    Ok(type_children.into_boxed_slice())
}

pub(super) fn attached_closure_children(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<(Box<[AttachedClosureParameter]>, Option<AttachedTypeRefNode>), SyntaxAccessError> {
    let ExpressionProjection::Closure(closure) = projection else {
        return Ok((Box::new([]), None));
    };
    let mut parameters = Vec::with_capacity(closure.parameters().len());
    for (ordinal, parameter) in closure.parameters().iter().enumerate() {
        let ordinal = u16::try_from(ordinal)
            .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        let nodes = syntax.children_with_role(SyntaxRole::Parameter(ordinal));
        let [node] = nodes.as_slice() else {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        };
        if node.kind() != SyntaxKind::ClosureParameter {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let patterns = node.children_with_role(SyntaxRole::ParameterPattern);
        let [pattern] = patterns.as_slice() else {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        };
        let pattern = FamilyNode::<PatternFamily>::new(pattern.clone())?.semantic()?;
        let pattern_source = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::ClosureParameter {
                        parameter: ordinal,
                        part: SyntaxClosureParameterPart::Pattern,
                    }
            })
            .map(|component| component.range())
            .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
        if pattern.whole_source_span().range() != pattern_source {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }
        let types = node.children_with_role(SyntaxRole::ParameterType);
        let ty = match (parameter.has_type(), types.as_slice()) {
            (false, []) => None,
            (true, [ty]) => {
                let ty = AttachedTypeRefNode::from_syntax(ty.clone())?;
                let type_source = components
                    .iter()
                    .find(|component| {
                        component.role()
                            == ExpressionComponentRole::ClosureParameter {
                                parameter: ordinal,
                                part: SyntaxClosureParameterPart::Type,
                            }
                    })
                    .map(|component| component.range())
                    .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
                if ty.whole_source_span().range() != type_source {
                    return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
                }
                Some(ty)
            }
            _ => {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
        };
        parameters.push(AttachedClosureParameter { pattern, ty });
    }

    let result_nodes = syntax.children_with_role(SyntaxRole::ReturnType);
    let result_type = match (closure.has_result_type(), result_nodes.as_slice()) {
        (false, []) => None,
        (true, [result]) if result.kind() == SyntaxKind::ReturnType => {
            let types = result.children_with_role(SyntaxRole::Type);
            let [ty] = types.as_slice() else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            let ty = AttachedTypeRefNode::from_syntax(ty.clone())?;
            let source = components
                .iter()
                .find(|component| component.role() == ExpressionComponentRole::ReturnType)
                .map(|component| component.range())
                .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            if ty.whole_source_span().range() != source {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            Some(ty)
        }
        _ => return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }),
    };
    Ok((parameters.into_boxed_slice(), result_type))
}

fn attached_record_children(
    syntax: &SyntaxNodeHandle,
    fields: &[SyntaxRecordField],
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<Box<[AttachedExpressionChild]>, SyntaxAccessError> {
    let field_nodes = syntax
        .children()
        .into_iter()
        .filter(|child| child.kind() == SyntaxKind::RecordField)
        .collect::<Vec<_>>();
    if field_nodes.len() != fields.len() {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }

    field_nodes
        .into_iter()
        .zip(fields)
        .enumerate()
        .filter_map(|(ordinal, (field_node, field))| {
            let ordinal = match u32::try_from(ordinal) {
                Ok(ordinal) => ordinal,
                Err(_) => {
                    return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                        id: syntax.id(),
                    }));
                }
            };
            let field_role = match u16::try_from(ordinal) {
                Ok(ordinal) => SyntaxRole::Field(ordinal),
                Err(_) => {
                    return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                        id: syntax.id(),
                    }));
                }
            };
            let whole =
                record_component_range(components, ordinal, ExpressionRecordFieldPart::Whole);
            let name_range =
                record_component_range(components, ordinal, ExpressionRecordFieldPart::Name);
            let names = field_node.children_with_role(SyntaxRole::Name);
            let [name_node] = names.as_slice() else {
                return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                    id: syntax.id(),
                }));
            };
            let name_kind_matches = match field.name() {
                Ok(_) => name_node.kind() == SyntaxKind::NameReference,
                Err(SyntaxNameIssue::Missing) => name_node.kind() == SyntaxKind::MissingName,
                Err(_) => name_node.kind() == SyntaxKind::NameReference,
            };
            if field_node.role() != field_role
                || whole.is_none_or(|whole| {
                    whole.start() != field_node.range().start()
                        || whole.end() > field_node.range().end()
                })
                || name_range != Some(name_node.range())
                || !name_kind_matches
            {
                return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                    id: syntax.id(),
                }));
            }

            match field {
                SyntaxRecordField::Explicit { value, .. } => {
                    let children = field_node.children_with_role(SyntaxRole::Initializer);
                    let [child] = children.as_slice() else {
                        return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                            id: syntax.id(),
                        }));
                    };
                    let Some(value_range) = record_component_range(
                        components,
                        ordinal,
                        ExpressionRecordFieldPart::Value,
                    ) else {
                        return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                            id: syntax.id(),
                        }));
                    };
                    if !ExpressionFamily::accepts(child.kind())
                        || !component_matches_semantic_child(syntax, child, value_range)
                    {
                        return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                            id: syntax.id(),
                        }));
                    }
                    Some(match value {
                        crate::expressions::SyntaxExpressionSlot::Authored
                            if child.kind() != SyntaxKind::MissingExpression =>
                        {
                            FamilyNode::<ExpressionFamily>::new(child.clone()).map(|expression| {
                                AttachedExpressionChild::Authored {
                                    ordinal,
                                    expression,
                                    source: syntax.source_span_for_range(value_range),
                                }
                            })
                        }
                        crate::expressions::SyntaxExpressionSlot::Missing
                            if child.kind() == SyntaxKind::MissingExpression =>
                        {
                            FamilyNode::<RecoveryFamily>::new(child.clone()).map(|recovery| {
                                AttachedExpressionChild::Missing { ordinal, recovery }
                            })
                        }
                        _ => {
                            Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })
                        }
                    })
                }
                SyntaxRecordField::Shorthand { .. } => {
                    if !field_node
                        .children()
                        .into_iter()
                        .all(|child| !ExpressionFamily::accepts(child.kind()))
                    {
                        return Some(Err(SyntaxAccessError::InvalidExpressionProjection {
                            id: syntax.id(),
                        }));
                    }
                    None
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn record_component_range(
    components: &[crate::expressions::PendingExpressionComponent],
    field: u32,
    part: ExpressionRecordFieldPart,
) -> Option<arcweft_source::SourceRange> {
    components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::RecordField { field, part })
        .map(|component| component.range())
}

fn semantic_child_role(projection: &ExpressionProjection, ordinal: u32) -> Option<SyntaxRole> {
    match projection {
        ExpressionProjection::Tuple(_) | ExpressionProjection::BracketSequence(_) => {
            Some(SyntaxRole::Element(ordinal))
        }
        ExpressionProjection::ArrayRepeat(_) if ordinal == 0 => Some(SyntaxRole::Element(0)),
        ExpressionProjection::ArrayRepeat(_) if ordinal == 1 => Some(SyntaxRole::Element(1)),
        ExpressionProjection::Select(_) if ordinal == 0 => Some(SyntaxRole::Target),
        ExpressionProjection::Index(_) if ordinal == 0 => Some(SyntaxRole::Target),
        ExpressionProjection::Index(_) if ordinal == 1 => Some(SyntaxRole::Argument(0)),
        ExpressionProjection::DialogueContentApplication(_) if ordinal == 0 => {
            Some(SyntaxRole::Target)
        }
        ExpressionProjection::PostfixBracket(_) if ordinal == 0 => Some(SyntaxRole::Target),
        ExpressionProjection::Pipe(_) if ordinal == 0 => Some(SyntaxRole::LeftOperand),
        ExpressionProjection::Pipe(_) if ordinal == 1 => Some(SyntaxRole::RightOperand),
        ExpressionProjection::Range { .. } if ordinal == 0 => Some(SyntaxRole::LeftOperand),
        ExpressionProjection::Range { .. } if ordinal == 1 => Some(SyntaxRole::RightOperand),
        ExpressionProjection::Binary { .. } if ordinal == 0 => Some(SyntaxRole::LeftOperand),
        ExpressionProjection::Binary { .. } if ordinal == 1 => Some(SyntaxRole::RightOperand),
        ExpressionProjection::Closure(_) if ordinal == 0 => Some(SyntaxRole::Body),
        ExpressionProjection::If { .. } if ordinal == 0 => Some(SyntaxRole::Condition),
        ExpressionProjection::If { .. } if ordinal == 1 => Some(SyntaxRole::ThenBranch),
        ExpressionProjection::If { .. } if ordinal == 2 => Some(SyntaxRole::ElseBranch),
        ExpressionProjection::IfLet { .. } if ordinal == 0 => Some(SyntaxRole::Scrutinee),
        ExpressionProjection::IfLet { .. } if ordinal == 1 => Some(SyntaxRole::Guard),
        ExpressionProjection::IfLet { .. } if ordinal == 2 => Some(SyntaxRole::ThenBranch),
        ExpressionProjection::IfLet { .. } if ordinal == 3 => Some(SyntaxRole::ElseBranch),
        ExpressionProjection::Match(_) if ordinal == 0 => Some(SyntaxRole::Scrutinee),
        ExpressionProjection::Try { .. }
        | ExpressionProjection::Await { .. }
        | ExpressionProjection::Borrow { .. }
        | ExpressionProjection::Dereference { .. }
        | ExpressionProjection::Unary { .. }
            if ordinal == 0 =>
        {
            Some(SyntaxRole::Operand)
        }
        _ => None,
    }
}

pub(super) fn component_matches_semantic_child(
    parent: &SyntaxNodeHandle,
    child: &SyntaxNodeHandle,
    component: arcweft_source::SourceRange,
) -> bool {
    let mut outer_group = None;
    let mut reached_parent = false;
    for ancestor in child.rowan().ancestors().skip(1) {
        if ancestor == *parent.rowan() {
            reached_parent = true;
            break;
        }
        if ancestor.kind().0 == SyntaxKind::DelimitedGroup as u16 {
            outer_group = Some(ancestor);
        }
    }
    if !reached_parent {
        return false;
    }
    match outer_group {
        Some(group) => {
            let range = group.text_range();
            component
                == arcweft_source::SourceRange::new(
                    usize::from(range.start()),
                    usize::from(range.end()),
                )
        }
        None => component == child.range(),
    }
}

fn component_range_for_slot(
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
    ordinal: u32,
) -> Option<arcweft_source::SourceRange> {
    let role = match projection {
        ExpressionProjection::Tuple(_) | ExpressionProjection::BracketSequence(_) => {
            ExpressionComponentRole::Element { ordinal }
        }
        ExpressionProjection::ArrayRepeat(_) if ordinal == 0 => {
            ExpressionComponentRole::RepeatValue
        }
        ExpressionProjection::ArrayRepeat(_) if ordinal == 1 => {
            ExpressionComponentRole::RepeatLength
        }
        ExpressionProjection::Select(_) if ordinal == 0 => ExpressionComponentRole::Target,
        ExpressionProjection::Index(_) if ordinal == 0 => ExpressionComponentRole::Target,
        ExpressionProjection::Index(_) if ordinal == 1 => ExpressionComponentRole::Index,
        ExpressionProjection::DialogueContentApplication(_) if ordinal == 0 => {
            ExpressionComponentRole::Target
        }
        ExpressionProjection::PostfixBracket(_) if ordinal == 0 => ExpressionComponentRole::Target,
        ExpressionProjection::Pipe(_) if ordinal == 0 => ExpressionComponentRole::LeftOperand,
        ExpressionProjection::Pipe(_) if ordinal == 1 => ExpressionComponentRole::RightOperand,
        ExpressionProjection::Range { .. } if ordinal == 0 => ExpressionComponentRole::RangeStart,
        ExpressionProjection::Range { .. } if ordinal == 1 => ExpressionComponentRole::RangeEnd,
        ExpressionProjection::Binary { .. } if ordinal == 0 => ExpressionComponentRole::LeftOperand,
        ExpressionProjection::Binary { .. } if ordinal == 1 => {
            ExpressionComponentRole::RightOperand
        }
        ExpressionProjection::Closure(_) if ordinal == 0 => ExpressionComponentRole::Body,
        ExpressionProjection::If { .. } if ordinal == 0 => ExpressionComponentRole::Condition,
        ExpressionProjection::If { .. } if ordinal == 1 => ExpressionComponentRole::ThenBranch,
        ExpressionProjection::If { .. } if ordinal == 2 => ExpressionComponentRole::ElseBranch,
        ExpressionProjection::IfLet { .. } if ordinal == 0 => ExpressionComponentRole::Scrutinee,
        ExpressionProjection::IfLet { .. } if ordinal == 1 => ExpressionComponentRole::Guard,
        ExpressionProjection::IfLet { .. } if ordinal == 2 => ExpressionComponentRole::ThenBranch,
        ExpressionProjection::IfLet { .. } if ordinal == 3 => ExpressionComponentRole::ElseBranch,
        ExpressionProjection::Match(_) if ordinal == 0 => ExpressionComponentRole::Scrutinee,
        ExpressionProjection::Record(_) | ExpressionProjection::RecordLiteral(_) => {
            ExpressionComponentRole::RecordField {
                field: ordinal,
                part: ExpressionRecordFieldPart::Value,
            }
        }
        ExpressionProjection::Try { .. }
        | ExpressionProjection::Await { .. }
        | ExpressionProjection::Borrow { .. }
        | ExpressionProjection::Dereference { .. }
        | ExpressionProjection::Unary { .. }
            if ordinal == 0 =>
        {
            ExpressionComponentRole::Operand
        }
        _ => return None,
    };
    components
        .iter()
        .find(|component| component.role() == role)
        .map(|component| component.range())
}

pub(super) fn validate_short_variant_shape(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Result<(), SyntaxAccessError> {
    let ExpressionProjection::ShortVariant(name) = projection else {
        return Ok(());
    };
    let names = syntax.children_with_role(SyntaxRole::Target);
    let [name_node] = names.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let child_matches = match name {
        Ok(_) => name_node.kind() == SyntaxKind::NameReference,
        Err(SyntaxNameIssue::Missing) => name_node.kind() == SyntaxKind::MissingName,
        Err(_) => name_node.kind() == SyntaxKind::NameReference,
    };
    let range_matches = components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::ShortVariantName)
        .is_some_and(|component| component.range() == name_node.range());
    if !child_matches || !range_matches {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }
    Ok(())
}
