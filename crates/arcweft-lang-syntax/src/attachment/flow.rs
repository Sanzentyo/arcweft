//! Revision-bound typed ownership for ordinary Flow declarations.
//!
//! This module consumes parser-owned projections and attached child identities.
//! It never rediscovers contract keywords, modes, or operands from source text.

use arcweft_source::SourceSpan;

use super::family::{ExpressionFamily, FamilyNode};
use super::node::{
    AssumeClauseKind, AstNode, BlockKind, CloseBraceKind, DeclarationPublicIdKind,
    DecreasesClauseKind, EffectsClauseKind, EnsuresClauseKind, ErrorNodeKind,
    FixedParameterGroupKind, FlowBodyKind, FlowItemKind, InvariantClauseKind, MissingBodyKind,
    MissingNameKind, ModifiesClauseKind, NameDefinitionKind, NoEffectClauseKind, OpenBraceKind,
    ReadsClauseKind, RequiresClauseKind, ReturnTypeKind,
};
use super::source_file::AttachedDelimiterState;
use super::thread_body::AttachedFlowStatementBody;
use super::{
    AttachedCallableReturn, AttachedExpressionNode, AttachedFixedParameterGroup,
    AttachedGenericParameterGroup, AttachedItemPrefix, AttachedWhereClause, SyntaxAccessError,
    SyntaxNodeHandle, TypedItemNode,
};
use crate::grammar::contract_projection::PendingFlowContractMode;
use crate::grammar::flow_projection::{
    PendingFlowDeclarationProjection, PendingFlowIdentity, PendingFlowPublicId,
    PendingFlowPublicIdForm,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::{AuthoredIdRoot, SyntaxIdRefIssue, SyntaxIdRefPart, SyntaxIdRefSyntax};
use crate::name::SyntaxName;

/// One exact entity-reference component bound to the accepted source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowIdComponent {
    part: SyntaxIdRefPart,
    source: SourceSpan,
}

impl AttachedFlowIdComponent {
    pub const fn part(&self) -> SyntaxIdRefPart {
        self.part
    }

    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// Authored or grammar-derived public-ID syntax without a source reader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowIdSyntax {
    Authored(SyntaxIdRefSyntax),
    DerivedFromEmptyMarker { marker_family: Option<SyntaxName> },
}

/// One Flow public-ID node and its parser-owned semantic projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowPublicId {
    syntax: AstNode<DeclarationPublicIdKind>,
    value: AttachedFlowIdSyntax,
    canonical_flow_family: bool,
    components: Box<[AttachedFlowIdComponent]>,
}

impl AttachedFlowPublicId {
    pub const fn syntax(&self) -> &AstNode<DeclarationPublicIdKind> {
        &self.syntax
    }

    pub const fn value(&self) -> &AttachedFlowIdSyntax {
        &self.value
    }

    pub const fn is_canonical_flow_family(&self) -> bool {
        self.canonical_flow_family
    }

    pub fn components(&self) -> &[AttachedFlowIdComponent] {
        &self.components
    }

    pub fn has_recovery(&self) -> bool {
        !self.canonical_flow_family
            || matches!(
                &self.value,
                AttachedFlowIdSyntax::Authored(value) if value.value().is_err()
            )
    }
}

/// One typed Flow name selected by the declaration grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowName {
    syntax: AstNode<NameDefinitionKind>,
    value: SyntaxName,
}

impl AttachedFlowName {
    pub const fn syntax(&self) -> &AstNode<NameDefinitionKind> {
        &self.syntax
    }

    pub const fn value(&self) -> &SyntaxName {
        &self.value
    }
}

/// The four admitted identity states of an ordinary Flow declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowIdentity {
    Name {
        name: AttachedFlowName,
    },
    PublicId {
        public_id: AttachedFlowPublicId,
    },
    PublicIdAndName {
        public_id: AttachedFlowPublicId,
        name: AttachedFlowName,
    },
    Missing {
        missing: AstNode<MissingNameKind>,
        insertion: SourceSpan,
        attempted_public_id: Option<AttachedFlowPublicId>,
    },
}

impl AttachedFlowIdentity {
    pub const fn name(&self) -> Option<&AttachedFlowName> {
        match self {
            Self::Name { name } | Self::PublicIdAndName { name, .. } => Some(name),
            Self::PublicId { .. } | Self::Missing { .. } => None,
        }
    }

    pub const fn public_id(&self) -> Option<&AttachedFlowPublicId> {
        match self {
            Self::PublicId { public_id } | Self::PublicIdAndName { public_id, .. } => {
                Some(public_id)
            }
            Self::Name { .. } | Self::Missing { .. } => None,
        }
    }

    pub const fn attempted_public_id(&self) -> Option<&AttachedFlowPublicId> {
        match self {
            Self::Missing {
                attempted_public_id,
                ..
            } => attempted_public_id.as_ref(),
            Self::Name { .. } | Self::PublicId { .. } | Self::PublicIdAndName { .. } => None,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Name { .. } => false,
            Self::PublicId { public_id } | Self::PublicIdAndName { public_id, .. } => {
                public_id.has_recovery()
            }
            Self::Missing { .. } => true,
        }
    }
}

/// Omitted Unit or one authored Flow return wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowReturnSyntax {
    Omitted,
    Authored(AttachedCallableReturn),
}

/// Shared callable signature owners aggregated for one ordinary Flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowSignature {
    generics: Option<AttachedGenericParameterGroup>,
    parameters: Option<AttachedFixedParameterGroup>,
    result: AttachedFlowReturnSyntax,
    where_clause: Option<AttachedWhereClause>,
    recovery: Box<[AttachedFlowSignatureRecovery]>,
    end: SourceSpan,
}

/// Typed malformed header syntax retained outside the one admitted signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowSignatureRecovery {
    /// A second fixed parameter group rejected by the non-curried Flow grammar.
    SecondParameterGroup {
        syntax: AstNode<ErrorNodeKind>,
        group: AstNode<FixedParameterGroupKind>,
    },
    /// Any other source-backed header recovery owned by the Flow signature.
    UnexpectedHeaderNode { syntax: AstNode<ErrorNodeKind> },
}

impl AttachedFlowSignatureRecovery {
    pub const fn syntax(&self) -> &AstNode<ErrorNodeKind> {
        match self {
            Self::SecondParameterGroup { syntax, .. } | Self::UnexpectedHeaderNode { syntax } => {
                syntax
            }
        }
    }

    pub const fn rejected_parameter_group(&self) -> Option<&AstNode<FixedParameterGroupKind>> {
        match self {
            Self::SecondParameterGroup { group, .. } => Some(group),
            Self::UnexpectedHeaderNode { .. } => None,
        }
    }
}

impl AttachedFlowSignature {
    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn parameters(&self) -> Option<&AttachedFixedParameterGroup> {
        self.parameters.as_ref()
    }

    pub const fn result(&self) -> &AttachedFlowReturnSyntax {
        &self.result
    }

    pub const fn where_clause(&self) -> Option<&AttachedWhereClause> {
        self.where_clause.as_ref()
    }

    pub fn recovery(&self) -> &[AttachedFlowSignatureRecovery] {
        &self.recovery
    }

    pub const fn end(&self) -> &SourceSpan {
        &self.end
    }

    pub fn has_recovery(&self) -> bool {
        self.generics
            .as_ref()
            .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || self
                .parameters
                .as_ref()
                .is_some_and(AttachedFixedParameterGroup::has_recovery)
            || matches!(
                &self.result,
                AttachedFlowReturnSyntax::Authored(result) if result.has_recovery()
            )
            || self
                .where_clause
                .as_ref()
                .is_some_and(AttachedWhereClause::has_recovery)
            || !self.recovery.is_empty()
    }
}

/// Present statement-only body or the exact required-body recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredFlowBody {
    Present(AttachedFlowStatementBody),
    Missing {
        syntax: AstNode<FlowBodyKind>,
        missing: AstNode<MissingBodyKind>,
        insertion: SourceSpan,
    },
}

/// Central attached owner for one ordinary Flow declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowDeclaration {
    syntax: AstNode<FlowItemKind>,
    prefix: AttachedItemPrefix,
    keyword: SourceSpan,
    identity: AttachedFlowIdentity,
    signature: AttachedFlowSignature,
    contracts: Box<[AttachedFlowContractClause]>,
    body: AttachedRequiredFlowBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedFlowDeclaration {
    pub const fn syntax(&self) -> &AstNode<FlowItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn keyword(&self) -> &SourceSpan {
        &self.keyword
    }

    pub const fn identity(&self) -> &AttachedFlowIdentity {
        &self.identity
    }

    pub const fn signature(&self) -> &AttachedFlowSignature {
        &self.signature
    }

    pub fn contracts(&self) -> &[AttachedFlowContractClause] {
        &self.contracts
    }

    pub const fn body(&self) -> &AttachedRequiredFlowBody {
        &self.body
    }

    pub fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.identity.has_recovery()
            || self.signature.has_recovery()
            || self
                .contracts
                .iter()
                .any(AttachedFlowContractClause::has_recovery)
            || match &self.body {
                AttachedRequiredFlowBody::Present(body) => body.has_recovery(),
                AttachedRequiredFlowBody::Missing { .. } => true,
            }
            || !self.trailing_recovery.is_empty()
    }
}

/// Closed Flow contract mode together with its exact authored token site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowContractMode {
    Default,
    Prove(SourceSpan),
    Check(SourceSpan),
    Debug(SourceSpan),
}

impl AttachedFlowContractMode {
    /// Returns the exact authored mode token, or `None` for the default mode.
    pub const fn source_span(&self) -> Option<&SourceSpan> {
        match self {
            Self::Default => None,
            Self::Prove(source) | Self::Check(source) | Self::Debug(source) => Some(source),
        }
    }
}

/// One scalar condition with its parser-selected closed mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowContractCondition {
    mode: AttachedFlowContractMode,
    expression: AttachedExpressionNode,
}

impl AttachedFlowContractCondition {
    pub const fn mode(&self) -> &AttachedFlowContractMode {
        &self.mode
    }

    pub const fn expression(&self) -> &AttachedExpressionNode {
        &self.expression
    }

    pub fn has_recovery(&self) -> bool {
        self.expression.projection().has_recovery()
    }
}

/// One source-ordered list payload, optionally enclosed in authored braces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowContractList {
    open: Option<AstNode<OpenBraceKind>>,
    operands: Box<[AttachedExpressionNode]>,
    close: Option<AstNode<CloseBraceKind>>,
}

impl AttachedFlowContractList {
    pub const fn open(&self) -> Option<&AstNode<OpenBraceKind>> {
        self.open.as_ref()
    }

    pub fn operands(&self) -> &[AttachedExpressionNode] {
        &self.operands
    }

    pub const fn close(&self) -> Option<&AstNode<CloseBraceKind>> {
        self.close.as_ref()
    }

    pub fn close_state(&self) -> Option<AttachedDelimiterState> {
        self.close.as_ref().map(AstNode::delimiter_state)
    }

    pub const fn is_braced(&self) -> bool {
        self.open.is_some()
    }

    pub fn has_recovery(&self) -> bool {
        self.close
            .as_ref()
            .is_some_and(|close| close.range().is_empty())
            || self
                .operands
                .iter()
                .any(|operand| operand.projection().has_recovery())
    }
}

/// Borrowed scalar or list payload of one Flow contract clause.
#[derive(Clone, Copy, Debug)]
pub enum AttachedFlowContractOperands<'a> {
    One(&'a AttachedExpressionNode),
    Many(&'a [AttachedExpressionNode]),
}

/// One heterogeneous Flow contract clause in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFlowContractClause {
    Requires {
        syntax: AstNode<RequiresClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        condition: AttachedFlowContractCondition,
    },
    Ensures {
        syntax: AstNode<EnsuresClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        condition: AttachedFlowContractCondition,
    },
    Invariant {
        syntax: AstNode<InvariantClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        condition: AttachedFlowContractCondition,
    },
    Assume {
        syntax: AstNode<AssumeClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        expression: AttachedExpressionNode,
    },
    Reads {
        syntax: AstNode<ReadsClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        operands: AttachedFlowContractList,
    },
    Effects {
        syntax: AstNode<EffectsClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        operands: AttachedFlowContractList,
    },
    NoEffect {
        syntax: AstNode<NoEffectClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        no_effect_keyword: SourceSpan,
        expression: AttachedExpressionNode,
    },
    Modifies {
        syntax: AstNode<ModifiesClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        operands: AttachedFlowContractList,
    },
    Decreases {
        syntax: AstNode<DecreasesClauseKind>,
        source_ordinal: u16,
        keyword: SourceSpan,
        expression: AttachedExpressionNode,
    },
}

impl AttachedFlowContractClause {
    fn from_syntax(
        syntax: SyntaxNodeHandle,
        expected_ordinal: u16,
    ) -> Result<Self, SyntaxAccessError> {
        if syntax.role() != SyntaxRole::ContractClause(expected_ordinal) {
            return Err(SyntaxAccessError::InvalidFlowContractShape { id: syntax.id() });
        }
        let projection = syntax
            .contract_clause_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingFlowContractProjection { id: syntax.id() })?;
        if !projection.ranges_are_valid_for(syntax.kind(), syntax.range()) {
            return Err(SyntaxAccessError::InvalidFlowContractProjection { id: syntax.id() });
        }
        let keyword = syntax.source_span_for_range(projection.clause_keyword());
        let mode = attached_mode(&syntax, projection.mode());

        match syntax.kind() {
            SyntaxKind::RequiresClause => Ok(Self::Requires {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                condition: scalar_condition(&syntax, mode)?,
            }),
            SyntaxKind::EnsuresClause => Ok(Self::Ensures {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                condition: scalar_condition(&syntax, mode)?,
            }),
            SyntaxKind::InvariantClause => Ok(Self::Invariant {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                condition: scalar_condition(&syntax, mode)?,
            }),
            SyntaxKind::AssumeClause => Ok(Self::Assume {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                expression: scalar_expression(&syntax)?,
            }),
            SyntaxKind::ReadsClause => Ok(Self::Reads {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                operands: list_payload(&syntax)?,
            }),
            SyntaxKind::EffectsClause => Ok(Self::Effects {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                operands: list_payload(&syntax)?,
            }),
            SyntaxKind::NoEffectClause => Ok(Self::NoEffect {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                no_effect_keyword: syntax.source_span_for_range(
                    projection.no_effect_keyword().ok_or(
                        SyntaxAccessError::InvalidFlowContractProjection { id: syntax.id() },
                    )?,
                ),
                expression: scalar_expression(&syntax)?,
            }),
            SyntaxKind::ModifiesClause => Ok(Self::Modifies {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                operands: list_payload(&syntax)?,
            }),
            SyntaxKind::DecreasesClause => Ok(Self::Decreases {
                syntax: syntax.cast()?,
                source_ordinal: expected_ordinal,
                keyword,
                expression: scalar_expression(&syntax)?,
            }),
            _ => Err(SyntaxAccessError::InvalidFlowContractShape { id: syntax.id() }),
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Requires { syntax, .. } => syntax.syntax(),
            Self::Ensures { syntax, .. } => syntax.syntax(),
            Self::Invariant { syntax, .. } => syntax.syntax(),
            Self::Assume { syntax, .. } => syntax.syntax(),
            Self::Reads { syntax, .. } => syntax.syntax(),
            Self::Effects { syntax, .. } => syntax.syntax(),
            Self::NoEffect { syntax, .. } => syntax.syntax(),
            Self::Modifies { syntax, .. } => syntax.syntax(),
            Self::Decreases { syntax, .. } => syntax.syntax(),
        }
    }

    pub const fn kind(&self) -> SyntaxKind {
        match self {
            Self::Requires { .. } => SyntaxKind::RequiresClause,
            Self::Ensures { .. } => SyntaxKind::EnsuresClause,
            Self::Invariant { .. } => SyntaxKind::InvariantClause,
            Self::Assume { .. } => SyntaxKind::AssumeClause,
            Self::Reads { .. } => SyntaxKind::ReadsClause,
            Self::Effects { .. } => SyntaxKind::EffectsClause,
            Self::NoEffect { .. } => SyntaxKind::NoEffectClause,
            Self::Modifies { .. } => SyntaxKind::ModifiesClause,
            Self::Decreases { .. } => SyntaxKind::DecreasesClause,
        }
    }

    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Requires { source_ordinal, .. }
            | Self::Ensures { source_ordinal, .. }
            | Self::Invariant { source_ordinal, .. }
            | Self::Assume { source_ordinal, .. }
            | Self::Reads { source_ordinal, .. }
            | Self::Effects { source_ordinal, .. }
            | Self::NoEffect { source_ordinal, .. }
            | Self::Modifies { source_ordinal, .. }
            | Self::Decreases { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub const fn keyword(&self) -> &SourceSpan {
        match self {
            Self::Requires { keyword, .. }
            | Self::Ensures { keyword, .. }
            | Self::Invariant { keyword, .. }
            | Self::Assume { keyword, .. }
            | Self::Reads { keyword, .. }
            | Self::Effects { keyword, .. }
            | Self::NoEffect { keyword, .. }
            | Self::Modifies { keyword, .. }
            | Self::Decreases { keyword, .. } => keyword,
        }
    }

    pub const fn mode(&self) -> Option<&AttachedFlowContractMode> {
        match self {
            Self::Requires { condition, .. }
            | Self::Ensures { condition, .. }
            | Self::Invariant { condition, .. } => Some(condition.mode()),
            Self::Assume { .. }
            | Self::Reads { .. }
            | Self::Effects { .. }
            | Self::NoEffect { .. }
            | Self::Modifies { .. }
            | Self::Decreases { .. } => None,
        }
    }

    pub const fn no_effect_keyword(&self) -> Option<&SourceSpan> {
        match self {
            Self::NoEffect {
                no_effect_keyword, ..
            } => Some(no_effect_keyword),
            _ => None,
        }
    }

    pub fn operands(&self) -> AttachedFlowContractOperands<'_> {
        match self {
            Self::Requires { condition, .. }
            | Self::Ensures { condition, .. }
            | Self::Invariant { condition, .. } => {
                AttachedFlowContractOperands::One(condition.expression())
            }
            Self::Assume { expression, .. }
            | Self::NoEffect { expression, .. }
            | Self::Decreases { expression, .. } => AttachedFlowContractOperands::One(expression),
            Self::Reads { operands, .. }
            | Self::Effects { operands, .. }
            | Self::Modifies { operands, .. } => {
                AttachedFlowContractOperands::Many(operands.operands())
            }
        }
    }

    pub const fn list(&self) -> Option<&AttachedFlowContractList> {
        match self {
            Self::Reads { operands, .. }
            | Self::Effects { operands, .. }
            | Self::Modifies { operands, .. } => Some(operands),
            _ => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Requires { condition, .. }
            | Self::Ensures { condition, .. }
            | Self::Invariant { condition, .. } => condition.has_recovery(),
            Self::Assume { expression, .. }
            | Self::NoEffect { expression, .. }
            | Self::Decreases { expression, .. } => expression.projection().has_recovery(),
            Self::Reads { operands, .. }
            | Self::Effects { operands, .. }
            | Self::Modifies { operands, .. } => operands.has_recovery(),
        }
    }
}

impl AstNode<FlowItemKind> {
    /// Binds identity, shared signature owners, contracts, and the required
    /// statement-only body to this exact immutable Flow syntax node.
    pub fn semantics(&self) -> Result<AttachedFlowDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .flow_declaration_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingFlowDeclarationProjection { id: self.id() })?;
        if !pending.ranges_are_valid_for(self.range()) {
            return Err(SyntaxAccessError::InvalidFlowDeclarationProjection { id: self.id() });
        }
        let identity = attach_flow_identity(&self.syntax(), pending.identity())?;
        let (signature, trailing_recovery) = attach_flow_signature(self, &pending)?;
        let contracts = self.contract_clauses()?.into_boxed_slice();
        let body = attach_flow_body(self)?;
        let signature_end = pending.signature_end().start();
        let body_start = flow_body_range(&body).start();
        if contracts.iter().any(|clause| {
            let range = clause.syntax().range();
            range.start() < signature_end || body_start < range.end()
        }) {
            return Err(SyntaxAccessError::InvalidFlowDeclarationShape { id: self.id() });
        }
        let item = TypedItemNode::Flow(self.clone());
        Ok(AttachedFlowDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            keyword: self.syntax().source_span_for_range(pending.keyword()),
            identity,
            signature,
            contracts,
            body,
            trailing_recovery,
        })
    }

    /// Returns the single heterogeneous contract sequence in authored order.
    pub fn contract_clauses(&self) -> Result<Vec<AttachedFlowContractClause>, SyntaxAccessError> {
        self.syntax()
            .ordered_children(SyntaxRoleClass::ContractClause)?
            .into_iter()
            .enumerate()
            .map(|(ordinal, syntax)| {
                let ordinal = u16::try_from(ordinal)
                    .map_err(|_| SyntaxAccessError::InvalidFlowContractShape { id: self.id() })?;
                AttachedFlowContractClause::from_syntax(syntax, ordinal)
            })
            .collect()
    }
}

fn attach_flow_identity(
    owner: &SyntaxNodeHandle,
    pending: &PendingFlowIdentity,
) -> Result<AttachedFlowIdentity, SyntaxAccessError> {
    let name = owner.optional_unique_child(SyntaxRole::Name)?;
    let public_id = owner.optional_unique_child(SyntaxRole::PublicId)?;
    match pending {
        PendingFlowIdentity::Name { value, source } => {
            if public_id.is_some() {
                return Err(invalid_flow(owner));
            }
            Ok(AttachedFlowIdentity::Name {
                name: attach_flow_name(owner, name, value, *source)?,
            })
        }
        PendingFlowIdentity::PublicId(pending) => {
            if name.is_some() {
                return Err(invalid_flow(owner));
            }
            Ok(AttachedFlowIdentity::PublicId {
                public_id: attach_flow_public_id(owner, public_id, pending)?,
            })
        }
        PendingFlowIdentity::PublicIdAndName {
            public_id: pending_public_id,
            name: value,
            name_source,
        } => Ok(AttachedFlowIdentity::PublicIdAndName {
            public_id: attach_flow_public_id(owner, public_id, pending_public_id)?,
            name: attach_flow_name(owner, name, value, *name_source)?,
        }),
        PendingFlowIdentity::Missing {
            insertion,
            public_id_recovery,
        } => {
            let missing = name
                .ok_or_else(|| invalid_flow(owner))?
                .cast::<MissingNameKind>()?;
            if missing.range() != *insertion {
                return Err(invalid_flow(owner));
            }
            let attempted_public_id = match public_id_recovery {
                Some(pending) => Some(attach_flow_public_id(owner, public_id, pending)?),
                None if public_id.is_none() => None,
                None => return Err(invalid_flow(owner)),
            };
            Ok(AttachedFlowIdentity::Missing {
                missing,
                insertion: owner.source_span_for_range(*insertion),
                attempted_public_id,
            })
        }
    }
}

fn attach_flow_name(
    owner: &SyntaxNodeHandle,
    syntax: Option<SyntaxNodeHandle>,
    value: &SyntaxName,
    source: arcweft_source::SourceRange,
) -> Result<AttachedFlowName, SyntaxAccessError> {
    let syntax = syntax
        .ok_or_else(|| invalid_flow(owner))?
        .cast::<NameDefinitionKind>()?;
    if syntax.range() != source {
        return Err(invalid_flow(owner));
    }
    Ok(AttachedFlowName {
        syntax,
        value: value.clone(),
    })
}

fn attach_flow_public_id(
    owner: &SyntaxNodeHandle,
    syntax: Option<SyntaxNodeHandle>,
    pending: &PendingFlowPublicId,
) -> Result<AttachedFlowPublicId, SyntaxAccessError> {
    let syntax = syntax
        .ok_or_else(|| invalid_flow(owner))?
        .cast::<DeclarationPublicIdKind>()?;
    if syntax.range() != pending.source() {
        return Err(invalid_flow(owner));
    }
    let value = match pending.form() {
        PendingFlowPublicIdForm::Authored => {
            if pending.is_canonical_flow_family() && !authored_id_has_flow_family(pending.syntax())
            {
                return Err(invalid_flow(owner));
            }
            AttachedFlowIdSyntax::Authored(pending.syntax().clone())
        }
        PendingFlowPublicIdForm::DerivedFromEmptyMarker { family } => {
            let shape = pending.syntax().shape();
            let expected_canonical = family
                .as_ref()
                .is_none_or(|family| family.as_str() == "flow");
            if !matches!(
                pending.syntax().value(),
                Err(SyntaxIdRefIssue::MissingSuffix)
            ) || shape.has_absolute_marker()
                || shape.parent_depth() != 0
                || shape.has_family() != family.is_some()
                || pending.is_canonical_flow_family() != expected_canonical
            {
                return Err(invalid_flow(owner));
            }
            AttachedFlowIdSyntax::DerivedFromEmptyMarker {
                marker_family: family.clone(),
            }
        }
    };
    let components = pending
        .components()
        .iter()
        .map(|component| AttachedFlowIdComponent {
            part: component.part(),
            source: owner.source_span_for_range(component.range()),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok(AttachedFlowPublicId {
        syntax,
        value,
        canonical_flow_family: pending.is_canonical_flow_family(),
        components,
    })
}

fn authored_id_has_flow_family(syntax: &SyntaxIdRefSyntax) -> bool {
    let Ok(reference) = syntax.value() else {
        return false;
    };
    match reference.root() {
        AuthoredIdRoot::Absolute { .. } | AuthoredIdRoot::Relative { .. } => {
            matches!(reference.segments(), [family, _, ..] if family.as_str() == "flow")
        }
        AuthoredIdRoot::FamilyRelative { family, .. } => family.as_str() == "flow",
    }
}

fn attach_flow_signature(
    owner: &AstNode<FlowItemKind>,
    pending: &PendingFlowDeclarationProjection,
) -> Result<(AttachedFlowSignature, Box<[AstNode<ErrorNodeKind>]>), SyntaxAccessError> {
    let generics = super::nominal::optional_generics(&owner.syntax())?;
    let parameters = owner
        .syntax()
        .optional_unique_child(SyntaxRole::ParameterGroup)?
        .map(|syntax| syntax.cast::<FixedParameterGroupKind>())
        .transpose()?
        .map(|syntax| {
            let mut next_parameter_ordinal = 0;
            syntax.callable_semantics(0, &mut next_parameter_ordinal)
        })
        .transpose()?;
    let result = owner
        .syntax()
        .optional_unique_child(SyntaxRole::ReturnType)?
        .map(|syntax| syntax.cast::<ReturnTypeKind>())
        .transpose()?
        .map(|syntax| syntax.callable_semantics())
        .transpose()?
        .map_or(
            AttachedFlowReturnSyntax::Omitted,
            AttachedFlowReturnSyntax::Authored,
        );
    let where_clauses = super::nominal::where_clauses(&owner.syntax())?;
    let where_clause = match where_clauses.into_vec().as_slice() {
        [] => None,
        [clause] => Some(clause.clone()),
        _ => return Err(invalid_flow(&owner.syntax())),
    };
    let signature_end = pending.signature_end().start();
    let (mut signature_recovery, trailing_recovery): (Vec<_>, Vec<_>) = owner
        .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
        .into_iter()
        .partition(|recovery| recovery.range().end() <= signature_end);
    if let Some(parameters) = &parameters {
        signature_recovery.extend(
            parameters
                .parameters()
                .iter()
                .flat_map(|parameter| parameter.recovery().iter().cloned()),
        );
    }
    signature_recovery.sort_by_key(|recovery| (recovery.range().start(), recovery.range().end()));
    if signature_recovery
        .windows(2)
        .any(|pair| pair[0].id() == pair[1].id())
    {
        return Err(invalid_flow(&owner.syntax()));
    }
    let signature_recovery = signature_recovery
        .into_iter()
        .map(|syntax| {
            syntax
                .optional_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)
                .map(|group| match group {
                    Some(group) => {
                        AttachedFlowSignatureRecovery::SecondParameterGroup { syntax, group }
                    }
                    None => AttachedFlowSignatureRecovery::UnexpectedHeaderNode { syntax },
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let end = owner
        .syntax()
        .source_span_for_range(pending.signature_end());
    if generics
        .as_ref()
        .is_some_and(|value| signature_end < value.syntax().range().end())
        || parameters
            .as_ref()
            .is_some_and(|value| signature_end < value.syntax().range().end())
        || matches!(
            &result,
            AttachedFlowReturnSyntax::Authored(value)
                if signature_end < value.syntax().range().end()
        )
        || where_clause
            .as_ref()
            .is_some_and(|value| signature_end < value.syntax().range().end())
    {
        return Err(invalid_flow(&owner.syntax()));
    }
    if parameters.as_ref().is_some_and(|group| {
        group
            .parameters()
            .iter()
            .any(|parameter| parameter.default().is_some())
    }) {
        return Err(invalid_flow(&owner.syntax()));
    }
    Ok((
        AttachedFlowSignature {
            generics,
            parameters,
            result,
            where_clause,
            recovery: signature_recovery.into_boxed_slice(),
            end,
        },
        trailing_recovery.into_boxed_slice(),
    ))
}

fn attach_flow_body(
    owner: &AstNode<FlowItemKind>,
) -> Result<AttachedRequiredFlowBody, SyntaxAccessError> {
    let syntax = owner.required_exact_child::<FlowBodyKind>(SyntaxRole::Body)?;
    let body = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid_flow(&owner.syntax()))?;
    match body.kind() {
        SyntaxKind::MissingBody => {
            let missing = body.cast::<MissingBodyKind>()?;
            if !missing.range().is_empty() {
                return Err(invalid_flow(&owner.syntax()));
            }
            Ok(AttachedRequiredFlowBody::Missing {
                insertion: missing.source_span(),
                syntax,
                missing,
            })
        }
        SyntaxKind::Block => {
            let block = body.cast::<BlockKind>()?;
            Ok(AttachedRequiredFlowBody::Present(
                AttachedFlowStatementBody::from_block(syntax, block)
                    .map_err(|_| invalid_flow(&owner.syntax()))?,
            ))
        }
        _ => Err(invalid_flow(&owner.syntax())),
    }
}

fn flow_body_range(body: &AttachedRequiredFlowBody) -> arcweft_source::SourceRange {
    match body {
        AttachedRequiredFlowBody::Present(body) => body.range(),
        AttachedRequiredFlowBody::Missing { syntax, .. } => syntax.range(),
    }
}

fn invalid_flow(owner: &SyntaxNodeHandle) -> SyntaxAccessError {
    SyntaxAccessError::InvalidFlowDeclarationShape { id: owner.id() }
}

fn attached_mode(
    syntax: &SyntaxNodeHandle,
    mode: PendingFlowContractMode,
) -> AttachedFlowContractMode {
    match mode {
        PendingFlowContractMode::Default => AttachedFlowContractMode::Default,
        PendingFlowContractMode::Prove(source) => {
            AttachedFlowContractMode::Prove(syntax.source_span_for_range(source))
        }
        PendingFlowContractMode::Check(source) => {
            AttachedFlowContractMode::Check(syntax.source_span_for_range(source))
        }
        PendingFlowContractMode::Debug(source) => {
            AttachedFlowContractMode::Debug(syntax.source_span_for_range(source))
        }
    }
}

fn scalar_condition(
    syntax: &SyntaxNodeHandle,
    mode: AttachedFlowContractMode,
) -> Result<AttachedFlowContractCondition, SyntaxAccessError> {
    Ok(AttachedFlowContractCondition {
        mode,
        expression: scalar_expression(syntax)?,
    })
}

fn scalar_expression(
    syntax: &SyntaxNodeHandle,
) -> Result<AttachedExpressionNode, SyntaxAccessError> {
    let operands = syntax.ordered_children(SyntaxRoleClass::ContractOperand)?;
    let [operand] = operands.as_slice() else {
        return Err(SyntaxAccessError::InvalidFlowContractShape { id: syntax.id() });
    };
    if syntax.child(SyntaxRole::OpenDelimiter).is_some()
        || syntax.child(SyntaxRole::CloseDelimiter).is_some()
    {
        return Err(SyntaxAccessError::InvalidFlowContractShape { id: syntax.id() });
    }
    FamilyNode::<ExpressionFamily>::new(operand.clone())?.semantic()
}

fn list_payload(syntax: &SyntaxNodeHandle) -> Result<AttachedFlowContractList, SyntaxAccessError> {
    let operands = syntax
        .ordered_children(SyntaxRoleClass::ContractOperand)?
        .into_iter()
        .map(|operand| FamilyNode::<ExpressionFamily>::new(operand)?.semantic())
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let open = syntax
        .optional_unique_child(SyntaxRole::OpenDelimiter)?
        .map(|open| open.cast())
        .transpose()?;
    let close = syntax
        .optional_unique_child(SyntaxRole::CloseDelimiter)?
        .map(|close| close.cast())
        .transpose()?;
    if open.is_some() != close.is_some() {
        return Err(SyntaxAccessError::InvalidFlowContractShape { id: syntax.id() });
    }
    Ok(AttachedFlowContractList {
        open,
        operands,
        close,
    })
}

#[cfg(test)]
mod tests;
