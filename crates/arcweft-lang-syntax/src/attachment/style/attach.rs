//! Binding of parser-owned Style projections to revision-bound syntax.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_source::SourceRange;

use crate::grammar::style_projection::{
    PendingStyleBodyProjection, PendingStyleEnvironmentClause, PendingStyleEnvironmentComparison,
    PendingStyleEnvironmentCondition, PendingStyleEnvironmentConditionRecovery,
    PendingStyleEnvironmentField, PendingStyleEnvironmentProjection, PendingStyleId,
    PendingStyleMemberProjection, PendingStyleName, PendingStylePropertyProjection,
    PendingStylePunctuation, PendingStyleRuleProjection, PendingStyleSelectorPart,
    PendingStyleSelectorProjection, PendingStyleSelectorRelation, PendingStyleSelectorSequence,
    PendingStyleTokenProjection, PendingStyleTypeAnnotation,
};
use crate::grammar::{SyntaxKind, SyntaxRole};

use crate::attachment::node::{
    AstNode, CloseBraceKind, CloseParenKind, ColonKind, EqualsKind, ErrorNodeKind, MissingBodyKind,
    MissingNameKind, NameReferenceKind, OpenBraceKind, OpenParenKind, StyleBodyKind,
    StyleEnvironmentBlockKind, StyleEnvironmentClauseKind, StyleEnvironmentConditionKind,
    StyleItemKind, StylePropertyDeclarationKind, StyleRuleKind, StyleSelectorKind,
    StyleSelectorSequenceKind, StyleTokenDeclarationKind,
};
use crate::attachment::{
    AstKind, AttachedExpressionNode, AttachedTypeRefNode, SyntaxAccessError, SyntaxNodeHandle,
    SyntaxNodeId, TypedItemNode,
};

use super::{
    AttachedStyleAssignment, AttachedStyleAssignmentState, AttachedStyleBody,
    AttachedStyleDeclaration, AttachedStyleEnvironment, AttachedStyleEnvironmentClause,
    AttachedStyleEnvironmentComparison, AttachedStyleEnvironmentCondition,
    AttachedStyleEnvironmentConditionRecovery, AttachedStyleEnvironmentField,
    AttachedStyleExpression, AttachedStyleId, AttachedStyleMember, AttachedStyleName,
    AttachedStylePredicate, AttachedStyleProperty, AttachedStyleRule, AttachedStyleRuleBody,
    AttachedStyleSelector, AttachedStyleSelectorPart, AttachedStyleSelectorRelation,
    AttachedStyleSelectorSequence, AttachedStyleToken, AttachedStyleTypeAnnotation, StyleIdForm,
    StylePropertyOperation, StyleSyntaxNameIssue,
};

impl AstNode<StyleItemKind> {
    /// Binds the sole parser-owned Style projection without reading source text.
    pub fn semantics(&self) -> Result<AttachedStyleDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .style_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingStyleProjection { id: self.id() })?;
        let declaration = AttachedStyleDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Style(self.clone()).attached_prefix()?,
            id: attach_id(self, &pending.id)?,
            body: attach_body(self, &pending.body, self.id())?,
            trailing_header_recovery: attach_optional_recovery(
                self,
                SyntaxRole::Recovery(0),
                pending.trailing_header_recovery,
                self.id(),
            )?,
        };
        if pending.has_recovery() && !declaration.has_recovery() {
            return Err(SyntaxAccessError::InvalidStyleProjection { id: self.id() });
        }
        Ok(declaration)
    }
}

fn attach_id(
    owner: &AstNode<StyleItemKind>,
    pending: &PendingStyleId,
) -> Result<AttachedStyleId, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Reference(0))?
        .ok_or(SyntaxAccessError::InvalidStyleProjection { id: owner.id() })?;
    match pending {
        PendingStyleId::Authored {
            value,
            source,
            form,
            canonical_style_family,
        } => {
            let expected = match form {
                StyleIdForm::Explicit => SyntaxKind::EntityReferenceExpression,
                StyleIdForm::Bare => SyntaxKind::NameDefinition,
            };
            validate_node(owner.id(), &syntax, expected, *source)?;
            if matches!(form, StyleIdForm::Explicit) {
                let expression = AttachedExpressionNode::from_syntax(syntax.clone())?;
                let crate::expressions::ExpressionProjection::EntityReference(projected) =
                    expression.projection()
                else {
                    return Err(SyntaxAccessError::InvalidStyleProjection { id: owner.id() });
                };
                let style_family =
                    crate::name::SyntaxName::try_new(DeclarationIdentityFamily::Style.prefix())
                        .expect("fixed Style family is an identifier");
                let (normalized, canonical) = projected.normalized_for_family(&style_family);
                if &normalized != value || canonical != *canonical_style_family {
                    return Err(SyntaxAccessError::InvalidStyleProjection { id: owner.id() });
                }
            }
            Ok(AttachedStyleId::Authored {
                syntax,
                reference: value.clone(),
                form: *form,
                canonical_style_family: *canonical_style_family,
            })
        }
        PendingStyleId::Invalid {
            value,
            source,
            authored_name,
        } => {
            validate_node(
                owner.id(),
                &syntax,
                if *authored_name {
                    SyntaxKind::NameDefinition
                } else {
                    SyntaxKind::ErrorNode
                },
                *source,
            )?;
            Ok(AttachedStyleId::Invalid {
                syntax,
                reference: value.clone(),
            })
        }
        PendingStyleId::Missing { value, insertion } => {
            validate_node(owner.id(), &syntax, SyntaxKind::MissingName, *insertion)?;
            Ok(AttachedStyleId::Missing {
                syntax: syntax.cast()?,
                reference: value.clone(),
            })
        }
    }
}

fn attach_body<K: AstKind>(
    owner: &AstNode<K>,
    pending: &PendingStyleBodyProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleBody, SyntaxAccessError> {
    match pending {
        PendingStyleBodyProjection::Missing => Ok(AttachedStyleBody::Missing(
            owner.required_exact_child::<MissingBodyKind>(SyntaxRole::Body)?,
        )),
        PendingStyleBodyProjection::Braced { members, closed } => {
            let syntax = owner.required_exact_child::<StyleBodyKind>(SyntaxRole::Body)?;
            let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == *closed {
                return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
            }
            let attached = members
                .iter()
                .map(|member| attach_member(&syntax, member, declaration))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(AttachedStyleBody::Braced {
                syntax,
                open,
                close,
                members: attached.into_boxed_slice(),
            })
        }
    }
}

fn attach_member(
    owner: &AstNode<StyleBodyKind>,
    pending: &PendingStyleMemberProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleMember, SyntaxAccessError> {
    let member_role = SyntaxRole::Element(pending.source_ordinal());
    let syntax = owner
        .syntax()
        .optional_unique_child(member_role)?
        .ok_or(SyntaxAccessError::InvalidStyleProjection { id: declaration })?;
    match pending {
        PendingStyleMemberProjection::Token(token)
            if syntax.kind() == SyntaxKind::StyleTokenDeclaration =>
        {
            Ok(AttachedStyleMember::Token(Box::new(attach_token(
                syntax.cast()?,
                token,
                declaration,
            )?)))
        }
        PendingStyleMemberProjection::Rule(rule) if syntax.kind() == SyntaxKind::StyleRule => Ok(
            AttachedStyleMember::Rule(Box::new(attach_rule(syntax.cast()?, rule, declaration)?)),
        ),
        PendingStyleMemberProjection::Environment(environment)
            if syntax.kind() == SyntaxKind::StyleEnvironmentBlock =>
        {
            Ok(AttachedStyleMember::Environment(Box::new(
                attach_environment(syntax.cast()?, environment, declaration)?,
            )))
        }
        PendingStyleMemberProjection::Recovery { source_ordinal }
            if syntax.kind() == SyntaxKind::ErrorNode =>
        {
            Ok(AttachedStyleMember::Error {
                source_ordinal: *source_ordinal,
                syntax: syntax.cast()?,
            })
        }
        _ => Err(SyntaxAccessError::InvalidStyleProjection { id: declaration }),
    }
}

fn attach_token(
    syntax: AstNode<StyleTokenDeclarationKind>,
    pending: &PendingStyleTokenProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleToken, SyntaxAccessError> {
    let name = attach_name(
        &syntax,
        SyntaxRole::Name,
        &pending.name,
        SyntaxKind::NameDefinition,
        declaration,
    )?;
    if name.token_id() != pending.id {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    let type_annotation = match &pending.type_annotation {
        PendingStyleTypeAnnotation::Absent => {
            if syntax.syntax().child(SyntaxRole::Colon).is_some()
                || syntax.syntax().child(SyntaxRole::Type).is_some()
            {
                return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
            }
            None
        }
        PendingStyleTypeAnnotation::Present { colon } => {
            let colon_node = syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?;
            validate_range(declaration, colon_node.range(), *colon)?;
            let ty = syntax
                .syntax()
                .optional_unique_child(SyntaxRole::Type)?
                .ok_or(SyntaxAccessError::InvalidStyleProjection { id: declaration })?;
            Some(AttachedStyleTypeAnnotation {
                colon: colon_node,
                value: AttachedTypeRefNode::from_syntax(ty)?,
            })
        }
    };
    Ok(AttachedStyleToken {
        name,
        id: pending.id.clone(),
        assignment: attach_assignment(&syntax, pending.assignment, declaration)?,
        value: attach_expression(&syntax, SyntaxRole::Initializer, declaration)?,
        syntax,
        source_ordinal: pending.source_ordinal,
        type_annotation,
        allowed_at_this_depth: pending.allowed_at_this_depth,
    })
}

fn attach_rule(
    syntax: AstNode<StyleRuleKind>,
    pending: &PendingStyleRuleProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleRule, SyntaxAccessError> {
    let selector_syntax = syntax.required_exact_child::<StyleSelectorKind>(SyntaxRole::Target)?;
    let body_syntax = syntax.required_exact_child::<StyleBodyKind>(SyntaxRole::Body)?;
    let open = body_syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = body_syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    if close.range().is_empty() == pending.body_closed {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    let declarations = pending
        .declarations
        .iter()
        .map(|property| attach_property(&body_syntax, property, declaration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AttachedStyleRule {
        selector: attach_selector(selector_syntax, &pending.selector, declaration)?,
        body: AttachedStyleRuleBody {
            syntax: body_syntax,
            open,
            close,
            declarations: declarations.into_boxed_slice(),
        },
        syntax,
        source_ordinal: pending.source_ordinal,
    })
}

fn attach_selector(
    syntax: AstNode<StyleSelectorKind>,
    pending: &PendingStyleSelectorProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleSelector, SyntaxAccessError> {
    let sequences = pending
        .sequences
        .iter()
        .map(|sequence| attach_selector_sequence(&syntax, sequence, declaration))
        .collect::<Result<Vec<_>, _>>()?;
    let recoveries = (0..pending.recovery_count)
        .map(|ordinal| syntax.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(ordinal)))
        .collect::<Result<Vec<_>, _>>()?;
    let missing = syntax.optional_exact_child::<MissingNameKind>(SyntaxRole::Name)?;
    if missing.is_some() != pending.missing {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(AttachedStyleSelector {
        syntax,
        sequences: sequences.into_boxed_slice(),
        recoveries: recoveries.into_boxed_slice(),
        missing,
    })
}

fn attach_selector_sequence(
    selector: &AstNode<StyleSelectorKind>,
    pending: &PendingStyleSelectorSequence,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleSelectorSequence, SyntaxAccessError> {
    let syntax = selector.required_exact_child::<StyleSelectorSequenceKind>(
        SyntaxRole::Element(pending.source_ordinal),
    )?;
    let relation = pending
        .relation
        .as_ref()
        .map(|relation| attach_selector_relation(selector, relation, declaration))
        .transpose()?;
    let element = pending
        .element
        .as_ref()
        .map(|name| {
            attach_name(
                &syntax,
                SyntaxRole::Name,
                name,
                SyntaxKind::NameReference,
                declaration,
            )
        })
        .transpose()?;
    let part = pending
        .part
        .as_ref()
        .map(|part| attach_selector_part(&syntax, part, declaration))
        .transpose()?;
    let predicates = pending
        .predicates
        .iter()
        .map(|predicate| attach_predicate(&syntax, predicate, declaration))
        .collect::<Result<Vec<_>, _>>()?;
    let recovery = syntax.optional_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
    if recovery.is_some() != pending.has_recovery {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(AttachedStyleSelectorSequence {
        syntax,
        source_ordinal: pending.source_ordinal,
        relation,
        element,
        part,
        predicates: predicates.into_boxed_slice(),
        recovery,
    })
}

fn attach_selector_relation(
    selector: &AstNode<StyleSelectorKind>,
    pending: &PendingStyleSelectorRelation,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleSelectorRelation, SyntaxAccessError> {
    validate_within(declaration, selector.range(), pending.source)?;
    Ok(AttachedStyleSelectorRelation {
        value: pending.value,
        source: selector.syntax().source_span_for_range(pending.source),
    })
}

fn attach_selector_part(
    sequence: &AstNode<StyleSelectorSequenceKind>,
    pending: &PendingStyleSelectorPart,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleSelectorPart, SyntaxAccessError> {
    validate_within(declaration, sequence.range(), pending.separator)?;
    Ok(AttachedStyleSelectorPart {
        separator: sequence.syntax().source_span_for_range(pending.separator),
        name: attach_name(
            sequence,
            SyntaxRole::Target,
            &pending.name,
            SyntaxKind::NameReference,
            declaration,
        )?,
    })
}

fn attach_predicate(
    sequence: &AstNode<StyleSelectorSequenceKind>,
    pending: &crate::grammar::style_projection::PendingStylePredicate,
    declaration: SyntaxNodeId,
) -> Result<AttachedStylePredicate, SyntaxAccessError> {
    validate_within(declaration, sequence.range(), pending.colon)?;
    Ok(AttachedStylePredicate {
        source_ordinal: pending.source_ordinal,
        colon: sequence.syntax().source_span_for_range(pending.colon),
        name: attach_name(
            sequence,
            SyntaxRole::Label(pending.source_ordinal),
            &pending.name,
            SyntaxKind::NameReference,
            declaration,
        )?,
    })
}

fn attach_property(
    owner: &AstNode<StyleBodyKind>,
    pending: &PendingStylePropertyProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleProperty, SyntaxAccessError> {
    let syntax = owner.required_exact_child::<StylePropertyDeclarationKind>(
        SyntaxRole::Element(pending.source_ordinal),
    )?;
    let append_keyword = if let Some(source) = pending.append_keyword {
        let keyword = syntax.required_exact_child::<NameReferenceKind>(SyntaxRole::Kind)?;
        validate_range(declaration, keyword.range(), source)?;
        Some(keyword)
    } else {
        if syntax.syntax().child(SyntaxRole::Kind).is_some() {
            return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
        }
        None
    };
    if append_keyword.is_some() != matches!(pending.operation, StylePropertyOperation::Append) {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(AttachedStyleProperty {
        name: attach_name(
            &syntax,
            SyntaxRole::Name,
            &pending.name,
            SyntaxKind::NameDefinition,
            declaration,
        )?,
        assignment: attach_assignment(&syntax, pending.assignment, declaration)?,
        value: attach_expression(&syntax, SyntaxRole::Initializer, declaration)?,
        syntax,
        source_ordinal: pending.source_ordinal,
        operation: pending.operation,
        append_keyword,
    })
}

fn attach_environment(
    syntax: AstNode<StyleEnvironmentBlockKind>,
    pending: &PendingStyleEnvironmentProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleEnvironment, SyntaxAccessError> {
    let condition =
        syntax.required_exact_child::<StyleEnvironmentConditionKind>(SyntaxRole::Condition)?;
    Ok(AttachedStyleEnvironment {
        intrinsic: attach_name(
            &syntax,
            SyntaxRole::Target,
            &pending.intrinsic,
            SyntaxKind::NameReference,
            declaration,
        )?,
        condition: attach_environment_condition(condition, &pending.condition, declaration)?,
        body: attach_body(&syntax, &pending.body, declaration)?,
        syntax,
        source_ordinal: pending.source_ordinal,
    })
}

fn attach_environment_condition(
    syntax: AstNode<StyleEnvironmentConditionKind>,
    pending: &PendingStyleEnvironmentCondition,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleEnvironmentCondition, SyntaxAccessError> {
    let open = syntax.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?;
    let close = syntax.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?;
    validate_delimiter(declaration, open.range(), pending.open)?;
    validate_delimiter(declaration, close.range(), pending.close)?;
    let clauses = pending
        .clauses
        .iter()
        .map(|clause| attach_environment_clause(&syntax, clause, declaration))
        .collect::<Result<Vec<_>, _>>()?;
    let recoveries = pending
        .recoveries
        .iter()
        .map(|recovery| attach_environment_condition_recovery(&syntax, recovery, declaration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AttachedStyleEnvironmentCondition {
        syntax,
        open,
        close,
        clauses: clauses.into_boxed_slice(),
        recoveries: recoveries.into_boxed_slice(),
    })
}

fn attach_environment_condition_recovery(
    condition: &AstNode<StyleEnvironmentConditionKind>,
    pending: &PendingStyleEnvironmentConditionRecovery,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleEnvironmentConditionRecovery, SyntaxAccessError> {
    let syntax = condition
        .required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(pending.source_ordinal))?;
    validate_range(declaration, syntax.range(), pending.source)?;
    Ok(AttachedStyleEnvironmentConditionRecovery {
        syntax,
        source_ordinal: pending.source_ordinal,
        issue: pending.issue,
    })
}

fn attach_environment_clause(
    condition: &AstNode<StyleEnvironmentConditionKind>,
    pending: &PendingStyleEnvironmentClause,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleEnvironmentClause, SyntaxAccessError> {
    let syntax = condition.required_exact_child::<StyleEnvironmentClauseKind>(
        SyntaxRole::Field(pending.source_ordinal),
    )?;
    let field = match &pending.field {
        PendingStyleEnvironmentField::Known { value, name } => {
            AttachedStyleEnvironmentField::Known {
                value: *value,
                name: attach_name(
                    &syntax,
                    SyntaxRole::Name,
                    name,
                    SyntaxKind::NameReference,
                    declaration,
                )?,
            }
        }
        PendingStyleEnvironmentField::Unsupported(name) => {
            AttachedStyleEnvironmentField::Unsupported(attach_name(
                &syntax,
                SyntaxRole::Name,
                name,
                SyntaxKind::NameReference,
                declaration,
            )?)
        }
        PendingStyleEnvironmentField::Missing(name) => {
            AttachedStyleEnvironmentField::Missing(attach_name(
                &syntax,
                SyntaxRole::Name,
                name,
                SyntaxKind::NameReference,
                declaration,
            )?)
        }
    };
    let comparison = match pending.comparison {
        PendingStyleEnvironmentComparison::Known { value, source } => {
            validate_within(declaration, syntax.range(), source)?;
            AttachedStyleEnvironmentComparison::Known {
                value,
                source: syntax.syntax().source_span_for_range(source),
            }
        }
        PendingStyleEnvironmentComparison::Unsupported { source } => {
            validate_within(declaration, syntax.range(), source)?;
            AttachedStyleEnvironmentComparison::Unsupported {
                source: syntax.syntax().source_span_for_range(source),
            }
        }
        PendingStyleEnvironmentComparison::Missing { insertion } => {
            validate_within(declaration, syntax.range(), insertion)?;
            AttachedStyleEnvironmentComparison::Missing {
                insertion: syntax.syntax().source_span_for_range(insertion),
            }
        }
    };
    Ok(AttachedStyleEnvironmentClause {
        value: attach_expression(&syntax, SyntaxRole::Value, declaration)?,
        syntax,
        source_ordinal: pending.source_ordinal,
        field,
        comparison,
    })
}

fn attach_name<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    pending: &PendingStyleName,
    authored_kind: SyntaxKind,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleName, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidStyleProjection { id: declaration })?;
    match pending {
        PendingStyleName::Authored {
            value,
            dotted_component_count,
            source,
        } => {
            validate_node(declaration, &syntax, authored_kind, *source)?;
            Ok(AttachedStyleName {
                syntax,
                value: value.clone(),
                dotted_component_count: *dotted_component_count,
            })
        }
        PendingStyleName::Missing { insertion } => {
            validate_node(declaration, &syntax, SyntaxKind::MissingName, *insertion)?;
            Ok(AttachedStyleName {
                syntax,
                value: Err(StyleSyntaxNameIssue::Missing),
                dotted_component_count: 0,
            })
        }
    }
}

fn attach_assignment<K: AstKind>(
    owner: &AstNode<K>,
    pending: PendingStylePunctuation,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleAssignment, SyntaxAccessError> {
    let equals = owner.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?;
    let (state, expected_equals, source, expects_unsupported) = match pending {
        PendingStylePunctuation::Authored(source) => (
            AttachedStyleAssignmentState::Authored,
            source,
            source,
            false,
        ),
        PendingStylePunctuation::Missing(insertion) => (
            AttachedStyleAssignmentState::Missing,
            insertion,
            insertion,
            false,
        ),
        PendingStylePunctuation::Unsupported(source) => (
            AttachedStyleAssignmentState::Unsupported,
            SourceRange::new(source.start(), source.start()),
            source,
            true,
        ),
    };
    validate_range(declaration, equals.range(), expected_equals)?;
    let unsupported = owner.optional_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
    if unsupported.is_some() != expects_unsupported {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    if let Some(unsupported) = &unsupported {
        validate_range(declaration, unsupported.range(), source)?;
    }
    validate_within(declaration, owner.range(), source)?;
    Ok(AttachedStyleAssignment {
        equals,
        source: owner.syntax().source_span_for_range(source),
        state,
        unsupported,
    })
}

fn attach_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    declaration: SyntaxNodeId,
) -> Result<AttachedStyleExpression, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidStyleProjection { id: declaration })?;
    if syntax.kind() == SyntaxKind::MissingExpression {
        Ok(AttachedStyleExpression::Missing(syntax.cast()?))
    } else {
        AttachedExpressionNode::from_syntax(syntax)
            .map(Box::new)
            .map(AttachedStyleExpression::Authored)
    }
}

fn attach_optional_recovery<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    expected: bool,
    declaration: SyntaxNodeId,
) -> Result<Option<AstNode<ErrorNodeKind>>, SyntaxAccessError> {
    let recovery = owner.optional_exact_child::<ErrorNodeKind>(role)?;
    if recovery.is_some() != expected {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(recovery)
}

fn validate_delimiter(
    declaration: SyntaxNodeId,
    actual: SourceRange,
    expected: PendingStylePunctuation,
) -> Result<(), SyntaxAccessError> {
    match expected {
        PendingStylePunctuation::Authored(range) => validate_range(declaration, actual, range),
        PendingStylePunctuation::Missing(insertion) => {
            validate_range(declaration, actual, insertion)
        }
        PendingStylePunctuation::Unsupported(_) => {
            Err(SyntaxAccessError::InvalidStyleProjection { id: declaration })
        }
    }
}

fn validate_node(
    declaration: SyntaxNodeId,
    node: &SyntaxNodeHandle,
    kind: SyntaxKind,
    range: SourceRange,
) -> Result<(), SyntaxAccessError> {
    if node.kind() != kind || node.range() != range {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(())
}

fn validate_range(
    declaration: SyntaxNodeId,
    actual: SourceRange,
    expected: SourceRange,
) -> Result<(), SyntaxAccessError> {
    if actual != expected {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(())
}

fn validate_within(
    declaration: SyntaxNodeId,
    owner: SourceRange,
    range: SourceRange,
) -> Result<(), SyntaxAccessError> {
    if range.start() < owner.start() || range.end() > owner.end() || range.start() > range.end() {
        return Err(SyntaxAccessError::InvalidStyleProjection { id: declaration });
    }
    Ok(())
}
