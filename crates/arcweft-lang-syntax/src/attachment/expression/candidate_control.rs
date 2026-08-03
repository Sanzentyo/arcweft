//! Typed control-expression views borrowed from one retained candidate graph.

use arcweft_source::{SourceRange, SourceSpan};

use super::{
    AttachedCandidateExpressionChild, AttachedCandidateNode, AttachedCandidatePatternProjection,
};
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxClosureParameterPart,
    SyntaxClosureParameterProjection, SyntaxClosureProjection, SyntaxExpressionSlot,
    SyntaxMatchArmPart, SyntaxMatchArmProjection, SyntaxMatchProjection,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::patterns::{
    PatternBindingSite, PatternComponentRole, PatternNodeStep, PatternSyntaxState,
    PatternTypeChildRelation,
};
use crate::types::TypeRefNodePath;

/// One immediate typed child of a candidate Pattern projection.
#[derive(Clone, Copy)]
pub enum AttachedCandidatePatternChild<'a> {
    Pattern {
        step: PatternNodeStep,
        projection: AttachedCandidatePatternProjection<'a>,
    },
    Type {
        relation: PatternTypeChildRelation,
        node: AttachedCandidateNode<'a>,
    },
}

impl<'a> AttachedCandidatePatternChild<'a> {
    /// Structural Pattern edge, when this is a Pattern child.
    pub const fn pattern_step(self) -> Option<PatternNodeStep> {
        match self {
            Self::Pattern { step, .. } => Some(step),
            Self::Type { .. } => None,
        }
    }

    /// Typed-binding Type relation, when this is a Type child.
    pub const fn type_relation(self) -> Option<PatternTypeChildRelation> {
        match self {
            Self::Type { relation, .. } => Some(relation),
            Self::Pattern { .. } => None,
        }
    }

    /// Typed Pattern projection selected by this edge.
    pub const fn pattern(self) -> Option<AttachedCandidatePatternProjection<'a>> {
        match self {
            Self::Pattern { projection, .. } => Some(projection),
            Self::Type { .. } => None,
        }
    }

    /// Typed Type node selected by this edge.
    pub const fn type_ref(self) -> Option<AttachedCandidateNode<'a>> {
        match self {
            Self::Type { node, .. } => Some(node),
            Self::Pattern { .. } => None,
        }
    }
}

impl<'a> AttachedCandidatePatternProjection<'a> {
    /// Accepted outer expression identity owning this candidate-only Pattern.
    pub fn source_owner_id(self) -> crate::attachment::SyntaxNodeId {
        self.owner.id()
    }

    /// Accepted outer snapshot that owns this candidate-only Pattern.
    pub fn snapshot_id(self) -> &'a crate::attachment::SyntaxSnapshotId {
        self.owner.snapshot_id()
    }

    /// Candidate-local node carrying this parser-owned Pattern projection.
    pub fn node(self) -> AttachedCandidateNode<'a> {
        AttachedCandidateNode::new(self.owner, self.graph, self.index)
    }

    /// Parser-owned Pattern recovery state.
    pub fn state(self) -> &'a PatternSyntaxState {
        self.value().state()
    }

    /// Complete binding inventory in deterministic authored preorder.
    pub fn binding_sites(self) -> &'a [PatternBindingSite] {
        self.projection.authored().binding_sites()
    }

    /// Exact whole-node source span in the accepted outer revision.
    ///
    /// # Panics
    ///
    /// Panics only if the parser-owned candidate violates the invariant that
    /// every retained Pattern node has a `Whole` source component.
    pub fn whole_source_span(self) -> SourceSpan {
        self.component(PatternComponentRole::Whole)
            .expect("every candidate Pattern owns its whole source component")
    }

    /// Exact source span for one parser-owned Pattern component.
    pub fn component(self, role: PatternComponentRole) -> Option<SourceSpan> {
        let range = *self
            .projection
            .authored()
            .source()
            .component_at(self.projection.path(), role)?;
        Some(self.owner.syntax().source_span_for_range(range))
    }

    /// Immediate typed Pattern and typed-binding Type children.
    pub fn children(self) -> Option<Vec<AttachedCandidatePatternChild<'a>>> {
        let mut children = self
            .value()
            .immediate_child_steps()
            .into_iter()
            .map(|step| {
                let path = self.projection.path().child(step);
                let index = self.graph.pattern_node(self.projection.tree(), &path)?;
                let node = AttachedCandidateNode::new(self.owner, self.graph, index);
                Some(AttachedCandidatePatternChild::Pattern {
                    step,
                    projection: node.pattern_projection()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        if let Some(type_child) = self.projection.authored().source().type_child_at(
            self.projection.path(),
            PatternTypeChildRelation::TypedBinding,
        ) {
            let index = self.graph.type_node(type_child.tree(), type_child.path())?;
            let node = AttachedCandidateNode::new(self.owner, self.graph, index);
            let projection = node.type_projection()?;
            if projection.path() != type_child.path() {
                return None;
            }
            children.push(AttachedCandidatePatternChild::Type {
                relation: type_child.relation(),
                node,
            });
        }
        Some(children)
    }
}

/// One source-ordered Closure parameter in a retained candidate graph.
#[derive(Clone)]
pub struct AttachedCandidateClosureParameter<'a> {
    node: AttachedCandidateNode<'a>,
    ordinal: u16,
    projection: &'a SyntaxClosureParameterProjection,
    pattern: AttachedCandidatePatternProjection<'a>,
    ty: Option<AttachedCandidateNode<'a>>,
    whole_source: SourceSpan,
    pattern_source: SourceSpan,
    colon_source: Option<SourceSpan>,
    type_source: Option<SourceSpan>,
}

impl<'a> AttachedCandidateClosureParameter<'a> {
    /// Candidate-local parameter wrapper selected by its typed ordinal role.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Source-ordered parameter ordinal.
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    /// Parser-owned parameter shape borrowed from the Closure projection.
    pub const fn projection(&self) -> &'a SyntaxClosureParameterProjection {
        self.projection
    }

    /// Exact Pattern root structurally owned by this parameter wrapper.
    pub const fn pattern(&self) -> AttachedCandidatePatternProjection<'a> {
        self.pattern
    }

    /// Optional Type root structurally owned by this parameter wrapper.
    pub const fn ty(&self) -> Option<AttachedCandidateNode<'a>> {
        self.ty
    }

    /// Exact source for one parameter component.
    pub const fn component(&self, part: SyntaxClosureParameterPart) -> Option<&SourceSpan> {
        match part {
            SyntaxClosureParameterPart::Whole => Some(&self.whole_source),
            SyntaxClosureParameterPart::Pattern => Some(&self.pattern_source),
            SyntaxClosureParameterPart::Colon => self.colon_source.as_ref(),
            SyntaxClosureParameterPart::Type => self.type_source.as_ref(),
        }
    }
}

/// Typed Closure view whose relations come from candidate graph ownership.
#[derive(Clone)]
pub struct AttachedCandidateClosure<'a> {
    node: AttachedCandidateNode<'a>,
    projection: &'a SyntaxClosureProjection,
    parameters: Box<[AttachedCandidateClosureParameter<'a>]>,
    result_type: Option<AttachedCandidateNode<'a>>,
    body: AttachedCandidateExpressionChild<'a>,
}

impl<'a> AttachedCandidateClosure<'a> {
    /// Candidate-local Closure expression node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Parser-owned Closure projection.
    pub const fn projection(&self) -> &'a SyntaxClosureProjection {
        self.projection
    }

    /// Source-ordered parameter views.
    pub fn parameters(&self) -> &[AttachedCandidateClosureParameter<'a>] {
        &self.parameters
    }

    /// Optional result Type root beneath the `ReturnType` wrapper.
    pub const fn result_type(&self) -> Option<AttachedCandidateNode<'a>> {
        self.result_type
    }

    /// Authored or recovered Closure body.
    pub const fn body(&self) -> &AttachedCandidateExpressionChild<'a> {
        &self.body
    }

    /// Exact Closure component span, with duplicate roles rejected.
    pub fn component(&self, role: ExpressionComponentRole) -> Option<SourceSpan> {
        self.node.unique_component_span(role)
    }
}

/// Typed candidate `IfLet` view preserving its binding-scope relations.
#[derive(Clone)]
pub struct AttachedCandidateIfLet<'a> {
    node: AttachedCandidateNode<'a>,
    pattern: AttachedCandidatePatternProjection<'a>,
    scrutinee: AttachedCandidateExpressionChild<'a>,
    guard: Option<AttachedCandidateExpressionChild<'a>>,
    then_branch: AttachedCandidateExpressionChild<'a>,
    else_branch: Option<AttachedCandidateExpressionChild<'a>>,
    else_source: SourceSpan,
}

impl<'a> AttachedCandidateIfLet<'a> {
    /// Candidate-local `IfLet` expression node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Exact binding Pattern root.
    pub const fn pattern(&self) -> AttachedCandidatePatternProjection<'a> {
        self.pattern
    }

    /// Scrutinee evaluated outside the binding scope.
    pub const fn scrutinee(&self) -> &AttachedCandidateExpressionChild<'a> {
        &self.scrutinee
    }

    /// Optional guard evaluated inside the binding scope.
    pub const fn guard(&self) -> Option<&AttachedCandidateExpressionChild<'a>> {
        self.guard.as_ref()
    }

    /// Required then branch, including its source-owned missing form.
    pub const fn then_branch(&self) -> &AttachedCandidateExpressionChild<'a> {
        &self.then_branch
    }

    /// Optional authored else branch evaluated outside the binding scope.
    pub const fn else_branch(&self) -> Option<&AttachedCandidateExpressionChild<'a>> {
        self.else_branch.as_ref()
    }

    /// Authored else span or exact zero-width omission site.
    pub const fn else_source_span(&self) -> &SourceSpan {
        &self.else_source
    }
}

/// One source-ordered Match arm with its own structural scope boundary.
#[derive(Clone)]
pub struct AttachedCandidateMatchArm<'a> {
    owner: AttachedCandidateNode<'a>,
    node: AttachedCandidateNode<'a>,
    ordinal: u32,
    projection: &'a SyntaxMatchArmProjection,
    pattern: AttachedCandidatePatternProjection<'a>,
    guard: Option<AttachedCandidateExpressionChild<'a>>,
    value: AttachedCandidateExpressionChild<'a>,
}

impl<'a> AttachedCandidateMatchArm<'a> {
    /// Candidate-local `MatchArm` wrapper.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Source-ordered arm ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Parser-owned arm shape borrowed from the outer `Match` projection.
    pub const fn projection(&self) -> &'a SyntaxMatchArmProjection {
        self.projection
    }

    /// Exact Pattern root structurally owned by this arm.
    pub const fn pattern(&self) -> AttachedCandidatePatternProjection<'a> {
        self.pattern
    }

    /// Optional authored or recovered guard inside this arm.
    pub const fn guard(&self) -> Option<&AttachedCandidateExpressionChild<'a>> {
        self.guard.as_ref()
    }

    /// Required authored or recovered arm value.
    pub const fn value(&self) -> &AttachedCandidateExpressionChild<'a> {
        &self.value
    }

    /// Exact source for one outer Match-arm component.
    pub fn component(&self, part: SyntaxMatchArmPart) -> Option<SourceSpan> {
        self.owner
            .unique_component_span(ExpressionComponentRole::MatchArm {
                arm: self.ordinal,
                part,
            })
    }
}

/// Typed Match view preserving one scope boundary per arm.
#[derive(Clone)]
pub struct AttachedCandidateMatch<'a> {
    node: AttachedCandidateNode<'a>,
    projection: &'a SyntaxMatchProjection,
    scrutinee: AttachedCandidateExpressionChild<'a>,
    arms: Box<[AttachedCandidateMatchArm<'a>]>,
}

impl<'a> AttachedCandidateMatch<'a> {
    /// Candidate-local Match expression node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Parser-owned Match projection.
    pub const fn projection(&self) -> &'a SyntaxMatchProjection {
        self.projection
    }

    /// Match scrutinee evaluated outside every arm scope.
    pub const fn scrutinee(&self) -> &AttachedCandidateExpressionChild<'a> {
        &self.scrutinee
    }

    /// Source-ordered arm views.
    pub fn arms(&self) -> &[AttachedCandidateMatchArm<'a>] {
        &self.arms
    }
}

#[derive(Clone, Copy)]
enum ExactCandidateChild<T> {
    Absent,
    Present(T),
}

impl<T> ExactCandidateChild<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Self::Absent => None,
            Self::Present(child) => Some(child),
        }
    }

    const fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

impl<'a> AttachedCandidateNode<'a> {
    fn closure_parameter(
        self,
        node: Self,
        ordinal: u16,
        projection: &'a SyntaxClosureParameterProjection,
    ) -> Option<AttachedCandidateClosureParameter<'a>> {
        if node.kind() != SyntaxKind::ClosureParameter
            || node.role() != SyntaxRole::Parameter(ordinal)
        {
            return None;
        }
        let component = |part| {
            self.unique_component_range(ExpressionComponentRole::ClosureParameter {
                parameter: ordinal,
                part,
            })
        };
        let whole = component(SyntaxClosureParameterPart::Whole)?;
        let pattern_source = component(SyntaxClosureParameterPart::Pattern)?;
        if node.source_span().range() != whole {
            return None;
        }
        let pattern_node = node.exact_required_child(SyntaxRole::ParameterPattern)?;
        let pattern = pattern_node.pattern_root()?;
        if pattern_node.source_span().range() != pattern_source {
            return None;
        }
        let ty = node
            .exact_optional_child(SyntaxRole::ParameterType)?
            .into_option();
        let (colon_source, type_source) = if projection.has_type() {
            let ty = ty?;
            if ty.type_projection()?.path() != &TypeRefNodePath::root() {
                return None;
            }
            let colon_source = self
                .owner
                .syntax()
                .source_span_for_range(component(SyntaxClosureParameterPart::Colon)?);
            let type_source = self
                .owner
                .syntax()
                .source_span_for_range(component(SyntaxClosureParameterPart::Type)?);
            if ty.source_span().range() != type_source.range() {
                return None;
            }
            (Some(colon_source), Some(type_source))
        } else {
            if ty.is_some() {
                return None;
            }
            (None, None)
        };
        Some(AttachedCandidateClosureParameter {
            node,
            ordinal,
            projection,
            pattern,
            ty,
            whole_source: self.owner.syntax().source_span_for_range(whole),
            pattern_source: self.owner.syntax().source_span_for_range(pattern_source),
            colon_source,
            type_source,
        })
    }

    /// Typed Closure view selected from exact candidate graph relations.
    pub fn closure_view(self) -> Option<AttachedCandidateClosure<'a>> {
        let ExpressionProjection::Closure(projection) = self.expression_projection()? else {
            return None;
        };
        let parameter_nodes = self
            .children()
            .filter(|child| matches!(child.role(), SyntaxRole::Parameter(_)))
            .collect::<Vec<_>>();
        if parameter_nodes.len() != projection.parameters().len() {
            return None;
        }
        let mut parameters = Vec::with_capacity(parameter_nodes.len());
        for (ordinal, (node, parameter)) in parameter_nodes
            .into_iter()
            .zip(projection.parameters())
            .enumerate()
        {
            parameters.push(self.closure_parameter(
                node,
                u16::try_from(ordinal).ok()?,
                parameter,
            )?);
        }

        let return_wrapper = self
            .exact_optional_child(SyntaxRole::ReturnType)?
            .into_option();
        let result_type = if projection.has_result_type() {
            let wrapper = return_wrapper?;
            if wrapper.kind() != SyntaxKind::ReturnType {
                return None;
            }
            let ty = wrapper.exact_required_child(SyntaxRole::Type)?;
            if ty.type_projection()?.path() != &TypeRefNodePath::root()
                || ty.source_span().range()
                    != self.unique_component_range(ExpressionComponentRole::ReturnType)?
            {
                return None;
            }
            Some(ty)
        } else {
            if return_wrapper.is_some() {
                return None;
            }
            None
        };

        let body = self.expression_child(
            SyntaxRole::Body,
            0,
            projection.body(),
            ExpressionComponentRole::Body,
        )?;
        Some(AttachedCandidateClosure {
            node: self,
            projection,
            parameters: parameters.into_boxed_slice(),
            result_type,
            body,
        })
    }

    /// Typed `IfLet` view selected from exact candidate graph relations.
    pub fn if_let_view(self) -> Option<AttachedCandidateIfLet<'a>> {
        let ExpressionProjection::IfLet {
            scrutinee,
            guard,
            then_branch,
            else_branch,
        } = self.expression_projection()?
        else {
            return None;
        };
        let pattern_node = self.exact_required_child(SyntaxRole::Pattern)?;
        let pattern = pattern_node.pattern_root()?;
        if pattern_node.source_span().range()
            != self.unique_component_range(ExpressionComponentRole::Pattern)?
        {
            return None;
        }
        let scrutinee = self.expression_child(
            SyntaxRole::Scrutinee,
            0,
            *scrutinee,
            ExpressionComponentRole::Scrutinee,
        )?;
        let guard = if let Some(slot) = guard {
            Some(self.expression_child(
                SyntaxRole::Guard,
                1,
                *slot,
                ExpressionComponentRole::Guard,
            )?)
        } else {
            if self.exact_optional_child(SyntaxRole::Guard)?.is_present() {
                return None;
            }
            None
        };
        let then_branch = self.expression_child(
            SyntaxRole::ThenBranch,
            2,
            *then_branch,
            ExpressionComponentRole::ThenBranch,
        )?;
        let else_source = self.unique_component_span(ExpressionComponentRole::ElseBranch)?;
        let else_branch = if let Some(slot) = else_branch {
            Some(self.expression_child(
                SyntaxRole::ElseBranch,
                3,
                *slot,
                ExpressionComponentRole::ElseBranch,
            )?)
        } else {
            if self
                .exact_optional_child(SyntaxRole::ElseBranch)?
                .is_present()
            {
                return None;
            }
            None
        };
        Some(AttachedCandidateIfLet {
            node: self,
            pattern,
            scrutinee,
            guard,
            then_branch,
            else_branch,
            else_source,
        })
    }

    /// Typed Match view selected from exact arm-wrapper relations.
    pub fn match_view(self) -> Option<AttachedCandidateMatch<'a>> {
        let ExpressionProjection::Match(projection) = self.expression_projection()? else {
            return None;
        };
        let scrutinee = self.expression_child(
            SyntaxRole::Scrutinee,
            0,
            projection.scrutinee(),
            ExpressionComponentRole::Scrutinee,
        )?;
        let arm_nodes = self
            .children()
            .filter(|child| matches!(child.role(), SyntaxRole::MatchArm(_)))
            .collect::<Vec<_>>();
        if arm_nodes.len() != projection.arms().len() {
            return None;
        }
        let mut arms = Vec::with_capacity(arm_nodes.len());
        for (ordinal, (node, arm)) in arm_nodes.into_iter().zip(projection.arms()).enumerate() {
            let ordinal = u32::try_from(ordinal).ok()?;
            if node.kind() != SyntaxKind::MatchArm || node.role() != SyntaxRole::MatchArm(ordinal) {
                return None;
            }
            let component = |part| {
                self.unique_component_range(ExpressionComponentRole::MatchArm {
                    arm: ordinal,
                    part,
                })
            };
            if node.source_span().range() != component(SyntaxMatchArmPart::Whole)? {
                return None;
            }
            let pattern_node = node.exact_required_child(SyntaxRole::Pattern)?;
            let pattern = pattern_node.pattern_root()?;
            if pattern_node.source_span().range() != component(SyntaxMatchArmPart::Pattern)? {
                return None;
            }
            let guard = if let Some(slot) = arm.guard() {
                Some(node.expression_child_with_source(
                    SyntaxRole::Guard,
                    ordinal,
                    slot,
                    ExpressionComponentRole::MatchArm {
                        arm: ordinal,
                        part: SyntaxMatchArmPart::Guard,
                    },
                    component(SyntaxMatchArmPart::Guard)?,
                )?)
            } else {
                if node.exact_optional_child(SyntaxRole::Guard)?.is_present() {
                    return None;
                }
                None
            };
            let value = node.expression_child_with_source(
                SyntaxRole::Body,
                ordinal,
                arm.value(),
                ExpressionComponentRole::MatchArm {
                    arm: ordinal,
                    part: SyntaxMatchArmPart::Value,
                },
                component(SyntaxMatchArmPart::Value)?,
            )?;
            arms.push(AttachedCandidateMatchArm {
                owner: self,
                node,
                ordinal,
                projection: arm,
                pattern,
                guard,
                value,
            });
        }
        Some(AttachedCandidateMatch {
            node: self,
            projection,
            scrutinee,
            arms: arms.into_boxed_slice(),
        })
    }

    /// Root Pattern projection directly selected by this candidate node.
    pub fn pattern_root(self) -> Option<AttachedCandidatePatternProjection<'a>> {
        let projection = self.pattern_projection()?;
        projection.path().steps().is_empty().then_some(projection)
    }

    fn exact_required_child(self, role: SyntaxRole) -> Option<Self> {
        self.exact_optional_child(role)?.into_option()
    }

    fn exact_optional_child(self, role: SyntaxRole) -> Option<ExactCandidateChild<Self>> {
        let mut matches = self.children().filter(|child| child.role() == role);
        let child = match matches.next() {
            Some(child) => ExactCandidateChild::Present(child),
            None => ExactCandidateChild::Absent,
        };
        matches.next().is_none().then_some(child)
    }

    fn unique_component_range(self, role: ExpressionComponentRole) -> Option<SourceRange> {
        let crate::expressions::PendingCandidateSemantic::Expression(projection) =
            self.pending().semantic()
        else {
            return None;
        };
        let mut matches = projection
            .components()
            .iter()
            .filter(|component| component.role() == role);
        let source = matches.next()?.range();
        matches.next().is_none().then_some(source)
    }

    fn unique_component_span(self, role: ExpressionComponentRole) -> Option<SourceSpan> {
        Some(
            self.owner
                .syntax()
                .source_span_for_range(self.unique_component_range(role)?),
        )
    }

    fn expression_child(
        self,
        role: SyntaxRole,
        ordinal: u32,
        slot: SyntaxExpressionSlot,
        component_role: ExpressionComponentRole,
    ) -> Option<AttachedCandidateExpressionChild<'a>> {
        let source = self.unique_component_range(component_role)?;
        self.expression_child_with_source(role, ordinal, slot, component_role, source)
    }

    fn expression_child_with_source(
        self,
        role: SyntaxRole,
        ordinal: u32,
        slot: SyntaxExpressionSlot,
        component_role: ExpressionComponentRole,
        source: SourceRange,
    ) -> Option<AttachedCandidateExpressionChild<'a>> {
        let node = self.exact_required_child(role)?;
        if node.source_span().range() != source {
            return None;
        }
        let source = self.owner.syntax().source_span_for_range(source);
        match (slot, node.kind(), node.expression_projection()) {
            (SyntaxExpressionSlot::Missing, SyntaxKind::MissingExpression, _) => {
                Some(AttachedCandidateExpressionChild::Missing {
                    ordinal,
                    component_role,
                    source,
                    node,
                })
            }
            (SyntaxExpressionSlot::Authored, kind, Some(ExpressionProjection::Error))
                if kind != SyntaxKind::MissingExpression =>
            {
                Some(AttachedCandidateExpressionChild::Recovered {
                    ordinal,
                    component_role,
                    source,
                    node,
                })
            }
            (SyntaxExpressionSlot::Authored, kind, Some(_))
                if kind != SyntaxKind::MissingExpression =>
            {
                Some(AttachedCandidateExpressionChild::Authored {
                    ordinal,
                    component_role,
                    source,
                    node,
                })
            }
            _ => None,
        }
    }
}
