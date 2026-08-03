//! Recursive global preorder for E34 candidate `TypeId` expectations.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateBlockTail, AttachedCandidateExpressionChild, AttachedCandidateGraph,
    AttachedCandidateIfElse, AttachedCandidateIfHead, AttachedCandidateKeywordStatement,
    AttachedCandidateMatchArmBody, AttachedCandidateMatchBody, AttachedCandidateNode,
    AttachedCandidatePatternChild, AttachedCandidatePatternProjection, AttachedCandidateStatement,
    AttachedCandidateStatementBlock, AttachedCandidateStatementExpression,
    AttachedCandidateUnsafeBody,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxCallArgumentPart,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeArgumentProjection,
    SyntaxCallTypeChildRole, SyntaxExpressionSlot, SyntaxPostfixBracketProjection,
};
use arcweft_lang_syntax::grammar::{SyntaxKind, SyntaxRole};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::patterns::{
    PatternNodeStep, PatternRecordFieldSyntax, PatternSyntaxKind, PatternTypeChildRelation,
    PatternVariantPayloadSyntax,
};
use arcweft_lang_syntax::types::{TypeRef, TypeRefNodeStep};

use crate::final_lowering::pattern_lowering::binding_plan::{
    RecordFieldDisposition, classify_record_fields,
};
use crate::identity::{ExprId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::HirSourceSite;

use super::{
    CandidateTypeChild, CandidateValidationCursor, candidate_role_map, source_index_has_typed_owner,
};

/// One exact candidate type expected from the recursive role-local preorder.
pub(in crate::source_index) struct CandidateTypeExpectation {
    pub(in crate::source_index) key: SyntheticKey,
    pub(in crate::source_index) source_site: HirSourceSite,
    pub(in crate::source_index) payload: TypeRef,
    pub(in crate::source_index) children: BTreeMap<TypeRefNodeStep, TypeId>,
}

/// Re-derives candidate `TypeId`s in the lowerer's exact recursive traversal.
pub(in crate::source_index) fn candidate_type_expectations(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    retained_style_expressions: &std::collections::BTreeSet<ExprId>,
) -> Option<BTreeMap<TypeId, CandidateTypeExpectation>> {
    let mut expected = BTreeMap::new();
    for outer in slots.prepared_live_ids::<ExprId>() {
        let metadata = slots.resolve_prepared(outer).ok()?;
        let HirOrigin::Source(source) = metadata.origin() else {
            continue;
        };
        let Some(attached) = super::candidate_attached_expression(
            parsed,
            source.syntax(),
            outer,
            retained_style_expressions,
        )?
        else {
            continue;
        };
        for (role, graph, dialogue) in [
            (
                SyntheticRole::PostfixIndexCandidateExpression,
                attached.ambiguous_index_candidate(),
                false,
            ),
            (
                SyntheticRole::DialogueContentCandidateExpression,
                attached.ambiguous_dialogue_candidate(),
                true,
            ),
        ] {
            let Some(graph) = graph else { continue };
            let mut cursor = CandidateTypeExpectationCursor {
                parsed,
                slots,
                outer,
                role,
                next_type: 0,
                expected: &mut expected,
            };
            if dialogue {
                cursor.walk_dialogue(graph)?;
            } else {
                cursor.walk_index(graph)?;
            }
        }
    }
    Some(expected)
}

struct CandidateTypeExpectationCursor<'a> {
    parsed: &'a ParsedSource,
    slots: &'a SlotSnapshot,
    outer: ExprId,
    role: SyntheticRole,
    next_type: u32,
    expected: &'a mut BTreeMap<TypeId, CandidateTypeExpectation>,
}

impl CandidateTypeExpectationCursor<'_> {
    fn walk_index(&mut self, graph: AttachedCandidateGraph<'_>) -> Option<()> {
        self.walk_expression(graph.primary()?)
    }

    fn walk_dialogue(&mut self, graph: AttachedCandidateGraph<'_>) -> Option<()> {
        for slot in graph.dialogue_expression_slots()? {
            if slot.slot() == SyntaxExpressionSlot::Authored {
                self.walk_expression(slot.node())?;
            }
        }
        Some(())
    }

    fn walk_expression(&mut self, node: AttachedCandidateNode<'_>) -> Option<()> {
        let projection = node.expression_projection()?;
        match projection {
            ExpressionProjection::Call(call) => return self.walk_call(node, call),
            ExpressionProjection::Closure(_) => {
                let closure = node.closure_view()?;
                for parameter in closure.parameters() {
                    self.walk_pattern(parameter.pattern())?;
                    if let Some(ty) = parameter.ty() {
                        self.walk_type(ty)?;
                    }
                }
                if let Some(ty) = closure.result_type() {
                    self.walk_type(ty)?;
                }
                self.walk_expression_child(closure.body())?;
                return Some(());
            }
            ExpressionProjection::IfLet { .. } => {
                let if_let = node.if_let_view()?;
                self.walk_pattern(if_let.pattern())?;
                self.walk_expression_child(if_let.scrutinee())?;
                if let Some(guard) = if_let.guard() {
                    self.walk_expression_child(guard)?;
                }
                self.walk_expression_child(if_let.then_branch())?;
                if let Some(else_branch) = if_let.else_branch() {
                    self.walk_expression_child(else_branch)?;
                }
                return Some(());
            }
            ExpressionProjection::Match(_) => {
                let match_expression = node.match_view()?;
                self.walk_expression_child(match_expression.scrutinee())?;
                for arm in match_expression.arms() {
                    self.walk_pattern(arm.pattern())?;
                    if let Some(guard) = arm.guard() {
                        self.walk_expression_child(guard)?;
                    }
                    self.walk_expression_child(arm.value())?;
                }
                return Some(());
            }
            ExpressionProjection::Block
            | ExpressionProjection::ComputationBlock(_)
            | ExpressionProjection::NamedBlock(_) => {
                let block = node.value_block_view()?;
                for statement in block.statements() {
                    self.walk_statement(*statement)?;
                }
                if let AttachedCandidateBlockTail::Expression(tail) = block.tail() {
                    self.walk_statement_expression(tail)?;
                }
                return Some(());
            }
            _ => {}
        }
        for child in node.semantic_expression_children() {
            if child.slot() == SyntaxExpressionSlot::Authored {
                self.walk_expression(child.node())?;
            }
        }
        if matches!(
            projection,
            ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous { .. })
        ) {
            self.walk_index(node.ambiguous_index_candidate()?)?;
            self.walk_dialogue(node.ambiguous_dialogue_candidate()?)?;
        }
        Some(())
    }

    fn walk_expression_child(
        &mut self,
        child: &AttachedCandidateExpressionChild<'_>,
    ) -> Option<()> {
        if child.slot() == SyntaxExpressionSlot::Authored {
            self.walk_expression(child.node())?;
        }
        Some(())
    }

    fn walk_statement(&mut self, statement: AttachedCandidateStatement<'_>) -> Option<()> {
        match statement.kind() {
            SyntaxKind::AssertionStatement => {
                for condition in statement.assertion_view()?.conditions() {
                    self.walk_statement_expression(*condition)?;
                }
            }
            SyntaxKind::AssignmentStatement | SyntaxKind::LifetimeSetStatement => {
                let assignment = statement.assignment_view()?;
                self.walk_statement_expression(assignment.target())?;
                self.walk_statement_expression(assignment.value())?;
            }
            SyntaxKind::ReturnStatement
            | SyntaxKind::YieldStatement
            | SyntaxKind::WaitStatement
            | SyntaxKind::CloseStatement
            | SyntaxKind::SelectStatement => {
                self.walk_statement_expression(statement.required_operand_view()?.operand())?;
            }
            SyntaxKind::LetStatement => {
                self.walk_statement_expression(
                    statement.required_expression(SyntaxRole::Initializer)?,
                )?;
                self.walk_pattern(statement.required_pattern(SyntaxRole::Pattern)?)?;
            }
            SyntaxKind::ExpressionStatement => self.walk_statement_expression(
                statement.required_expression(SyntaxRole::Initializer)?,
            )?,
            SyntaxKind::OutStatement
            | SyntaxKind::GotoStatement
            | SyntaxKind::DeferStatement
            | SyntaxKind::SignalStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement => match statement.keyword_statement_view()? {
                AttachedCandidateKeywordStatement::Out { value, .. } => {
                    self.walk_statement_expression(value)?;
                }
                AttachedCandidateKeywordStatement::Goto { target, .. } => {
                    self.walk_statement_expression(target)?;
                }
                AttachedCandidateKeywordStatement::Defer { expression, .. } => {
                    self.walk_statement_expression(expression)?;
                }
                AttachedCandidateKeywordStatement::Signal { target, value, .. } => {
                    self.walk_statement_expression(target)?;
                    self.walk_statement_expression(value)?;
                }
                AttachedCandidateKeywordStatement::Break { value, .. } => {
                    if let Some(value) = value {
                        self.walk_statement_expression(value)?;
                    }
                }
                AttachedCandidateKeywordStatement::Continue { .. } => {}
            },
            SyntaxKind::ProofCallStatement => {
                self.walk_statement_expression(statement.required_expression(SyntaxRole::Callee)?)?;
            }
            SyntaxKind::IfStatement => {
                let conditional = statement.if_view()?;
                match conditional.head() {
                    AttachedCandidateIfHead::Condition(condition) => {
                        self.walk_statement_expression(*condition)?;
                    }
                    AttachedCandidateIfHead::Let {
                        pattern,
                        scrutinee,
                        guard,
                    } => {
                        self.walk_statement_expression(*scrutinee)?;
                        self.walk_pattern(*pattern)?;
                        if let Some(guard) = guard {
                            self.walk_statement_expression(*guard)?;
                        }
                    }
                }
                self.walk_statement_block(conditional.then_branch())?;
                match conditional.else_branch() {
                    None => {}
                    Some(AttachedCandidateIfElse::Block(block)) => {
                        self.walk_statement_block(block)?;
                    }
                    Some(AttachedCandidateIfElse::If(nested)) => {
                        self.walk_statement(*nested)?;
                    }
                }
            }
            SyntaxKind::MatchStatement => {
                let matched = statement.match_view()?;
                self.walk_statement_expression(matched.scrutinee())?;
                if let AttachedCandidateMatchBody::Block { arms, .. } = matched.body() {
                    for arm in arms {
                        self.walk_pattern(arm.pattern())?;
                        if let Some(guard) = arm.guard() {
                            self.walk_statement_expression(guard)?;
                        }
                        match arm.body() {
                            AttachedCandidateMatchArmBody::Expression(expression) => {
                                self.walk_statement_expression(*expression)?;
                            }
                            AttachedCandidateMatchArmBody::Block(block) => {
                                self.walk_statement_block(block)?;
                            }
                        }
                    }
                }
            }
            SyntaxKind::UnsafeLifetimeStatement => {
                let audit = statement.unsafe_lifetime_view()?;
                if let Some(reason) = audit.reason() {
                    self.walk_statement_expression(reason)?;
                }
                if let AttachedCandidateUnsafeBody::Block(block) = audit.body() {
                    self.walk_statement_block(block)?;
                }
            }
            SyntaxKind::ErrorStatement => {}
            _ => return None,
        }
        Some(())
    }

    fn walk_statement_block(&mut self, block: &AttachedCandidateStatementBlock<'_>) -> Option<()> {
        for statement in block.statements() {
            self.walk_statement(*statement)?;
        }
        Some(())
    }

    fn walk_statement_expression(
        &mut self,
        expression: AttachedCandidateStatementExpression<'_>,
    ) -> Option<()> {
        match expression {
            AttachedCandidateStatementExpression::Authored(node)
            | AttachedCandidateStatementExpression::Recovered(node) => self.walk_expression(node),
            AttachedCandidateStatementExpression::Missing(_) => Some(()),
        }
    }

    fn walk_pattern(&mut self, pattern: AttachedCandidatePatternProjection<'_>) -> Option<()> {
        let mut children = BTreeMap::new();
        let mut typed_binding = None;
        for child in pattern.children()? {
            match child {
                AttachedCandidatePatternChild::Pattern { step, projection } => {
                    if children.insert(step, projection).is_some() {
                        return None;
                    }
                }
                AttachedCandidatePatternChild::Type { relation, node } => {
                    if relation != PatternTypeChildRelation::TypedBinding
                        || typed_binding.replace(node).is_some()
                    {
                        return None;
                    }
                }
            }
        }
        let walk_child =
            |this: &mut Self, step| -> Option<()> { this.walk_pattern(*children.get(&step)?) };
        match pattern.value().kind() {
            PatternSyntaxKind::Variant(variant) => {
                if matches!(
                    variant.payload(),
                    PatternVariantPayloadSyntax::Resolved(_)
                        | PatternVariantPayloadSyntax::Recovered { value: Some(_), .. }
                ) {
                    walk_child(self, PatternNodeStep::VariantPayload)?;
                }
            }
            PatternSyntaxKind::Tuple(elements) | PatternSyntaxKind::Or(elements) => {
                for ordinal in 0..elements.len() {
                    walk_child(self, PatternNodeStep::Element(u32::try_from(ordinal).ok()?))?;
                }
            }
            PatternSyntaxKind::Record(record) => {
                for (ordinal, (field, disposition)) in record
                    .fields()
                    .iter()
                    .zip(classify_record_fields(record.fields()).ok()?)
                    .enumerate()
                {
                    if matches!(
                        (field, disposition),
                        (
                            PatternRecordFieldSyntax::Explicit { .. },
                            RecordFieldDisposition::Explicit { .. }
                        )
                    ) {
                        walk_child(
                            self,
                            PatternNodeStep::RecordField(u32::try_from(ordinal).ok()?),
                        )?;
                    }
                }
            }
            PatternSyntaxKind::BracketSequence(sequence) => {
                for ordinal in 0..sequence.elements().len() {
                    walk_child(self, PatternNodeStep::Element(u32::try_from(ordinal).ok()?))?;
                }
            }
            PatternSyntaxKind::WholeBinding { .. } => {
                walk_child(self, PatternNodeStep::NestedPattern)?;
            }
            PatternSyntaxKind::TypedBinding(_) => {
                self.walk_type(typed_binding?)?;
            }
            PatternSyntaxKind::Binding(_)
            | PatternSyntaxKind::MutableBinding(_)
            | PatternSyntaxKind::Literal(_)
            | PatternSyntaxKind::EntityReference(_)
            | PatternSyntaxKind::Discard
            | PatternSyntaxKind::Error => {}
        }
        Some(())
    }

    fn walk_call(
        &mut self,
        node: AttachedCandidateNode<'_>,
        call: &SyntaxCallProjection,
    ) -> Option<()> {
        let mut children = candidate_role_map(
            node.semantic_expression_children()
                .map(|child| (child.component_role(), child)),
        )?;
        let mut types = candidate_role_map(
            node.direct_semantic_type_roots()
                .map(|root| (root.role(), root)),
        )?;
        let mut walk_child = |this: &mut Self, role| -> Option<()> {
            let child = children.remove(&role)?;
            if child.slot() == SyntaxExpressionSlot::Authored {
                this.walk_expression(child.node())?;
            }
            Some(())
        };
        let mut walk_type = |this: &mut Self, role| -> Option<()> {
            let root = types.remove(&role)?;
            this.walk_type(root.node()).map(|_| ())
        };
        match call {
            SyntaxCallProjection::CallbackBlock(_) => {
                walk_child(self, ExpressionComponentRole::CallCallee)?;
                walk_child(
                    self,
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Value,
                    },
                )?;
            }
            SyntaxCallProjection::Parenthesized(call) => {
                match call.callee() {
                    SyntaxCallCalleeProjection::Ordinary => {
                        walk_child(self, ExpressionComponentRole::CallCallee)?;
                    }
                    SyntaxCallCalleeProjection::UnresolvedDot { .. } => {
                        walk_child(self, ExpressionComponentRole::CallAssociatedReceiver)?;
                        walk_type(self, SyntaxCallTypeChildRole::DotNominalReceiver)?;
                    }
                    SyntaxCallCalleeProjection::Associated { .. } => {
                        walk_type(self, SyntaxCallTypeChildRole::AssociatedReceiver)?;
                    }
                }
                if let Some(application) = call.explicit_type_application() {
                    for (position, argument) in application.arguments().iter().enumerate() {
                        if !matches!(argument, SyntaxCallTypeArgumentProjection::Missing) {
                            walk_type(
                                self,
                                SyntaxCallTypeChildRole::ExplicitCallTypeArgument {
                                    ordinal: u16::try_from(position).ok()?,
                                },
                            )?;
                        }
                    }
                }
                for position in 0..call.arguments().len() {
                    walk_child(
                        self,
                        ExpressionComponentRole::CallArgument {
                            argument: u16::try_from(position).ok()?,
                            part: SyntaxCallArgumentPart::Value,
                        },
                    )?;
                }
            }
        }
        (children.is_empty() && types.is_empty()).then_some(())
    }

    fn walk_type(&mut self, node: AttachedCandidateNode<'_>) -> Option<TypeId> {
        let projection = node.type_projection()?;
        let ordinal = self.next_type;
        self.next_type = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let owner = self
            .slots
            .resolve_prepared_synthetic::<TypeId>(key)
            .ok()??;
        let source_site =
            HirSourceSite::from_attached_span(self.parsed.document(), &node.source_span()).ok()?;
        let mut children = BTreeMap::new();
        for child in node.direct_semantic_type_children() {
            let child_owner = self.walk_type(child.node())?;
            if children.insert(child.step(), child_owner).is_some() {
                return None;
            }
        }
        if self
            .expected
            .insert(
                owner,
                CandidateTypeExpectation {
                    key,
                    source_site,
                    payload: projection.value().clone(),
                    children,
                },
            )
            .is_some()
        {
            return None;
        }
        Some(owner)
    }
}

impl CandidateValidationCursor<'_> {
    pub(super) fn validate_type(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
    ) -> Option<CandidateTypeChild> {
        node.type_projection()?;
        let ordinal = self.next_type;
        self.next_type = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let id = self
            .slots
            .resolve_prepared_synthetic::<TypeId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(id).ok()?;
        let payload = self.types.resolve_prepared(self.slots, id).ok()?;
        let expected = self.type_expectations.get(&id)?;
        if expected.key != key
            || metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != &expected.source_site
            || payload.scope() != scope
            || source_index_has_typed_owner(self.index, SyntheticOwner::Type(id))
            || !self.expected.types.insert(id)
        {
            return None;
        }
        for child in node.direct_semantic_type_children() {
            let nested = self.validate_type(child.node(), scope)?;
            if expected.children.get(&child.step()) != Some(&nested.id) {
                return None;
            }
        }
        Some(CandidateTypeChild {
            id,
            poisoned: payload.is_poisoned(),
        })
    }
}
