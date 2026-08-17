use std::collections::BTreeSet;
use std::sync::Arc;

use arcweft_lang_syntax::{
    attachment::{
        AttachedExpressionNode, AttachedFunctionBody, AttachedPathRoot, AttachedRequiredFlowBody,
        AttachedRequiredNestedThreadFlowBody, AttachedRequiredThreadExpressionBody,
        AttachedStyleBody, AttachedStyleEnvironment, AttachedStyleEnvironmentClause,
        AttachedStyleExpression, AttachedStyleMember, AttachedThreadFlowItem, BlockTailNode,
        IfStatementElseNode, IfStatementHeadNode, LetInitializerNode, MatchStatementArmBodyNode,
        MatchStatementExpressionNode, RequiredStatementExpressionNode, SyntaxAccessError,
        TypedItemNode,
        node::{
            AssertionStatementKind, AssignmentStatementKind, BlockKind, BreakStatementKind,
            CloseStatementKind, DeferStatementKind, ExpressionStatementKind, GotoStatementKind,
            IfStatementKind, LetStatementKind, LifetimeSetStatementKind, MatchStatementKind,
            OutStatementKind, ReturnStatementKind, SignalStatementKind, WaitStatementKind,
            YieldStatementKind,
        },
    },
    expressions::{
        ExpressionComponentRole, ExpressionProjection, SyntaxBuiltinRichTextTag,
        SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
        SyntaxRichTextArgumentProjection, SyntaxRichTextTagIdentity, SyntaxRichTextTagSourcePart,
    },
    grammar::SyntaxKind,
    incremental::{ParsedSource, SyntaxDatabase},
    literal::{SyntaxLiteralValue, UnitNumberSuffix},
    parser::ParseOptions,
};
use arcweft_source::{SourceDocument, SourceRange, identity::SourceSnapshotId};

use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingDiagnostic, ToolingEditReport, ToolingError};

mod view;

/// Formats an exact source document while preserving authoring sugar by default.
pub fn format_document(
    document: Arc<SourceDocument>,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let mut syntax =
        SyntaxDatabase::try_new().map_err(|error| ToolingError::SyntaxDatabaseUnavailable {
            message: error.to_string(),
        })?;
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
        .map_err(|error| ToolingError::SyntaxAttachmentFailed {
            message: error.to_string(),
        })?;
    let source = parsed.source();
    let mut edits =
        view::canonical_edits(source, &parsed).map_err(|error| syntax_attachment_error(&error))?;
    edits.extend(
        canonical_environment_edits(source, &parsed)
            .map_err(|error| syntax_attachment_error(&error))?,
    );
    if options.canonical_rich_text {
        edits.extend(
            canonical_rich_text_edits(source, &parsed)
                .map_err(|error| syntax_attachment_error(&error))?,
        );
    }
    let mut report = report_from_edits(source, edits)?;
    report.diagnostics = parsed
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let range = diagnostic.primary().range();
            ToolingDiagnostic::syntax(diagnostic.message(), range.start(), range.end())
        })
        .collect();
    Ok(report)
}

fn syntax_attachment_error(error: &SyntaxAccessError) -> ToolingError {
    ToolingError::SyntaxAttachmentFailed {
        message: error.to_string(),
    }
}

fn canonical_environment_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Result<Vec<TextEdit>, SyntaxAccessError> {
    let mut edits = Vec::new();
    for item in parsed.items()? {
        let TypedItemNode::Style(style) = item else {
            continue;
        };
        collect_style_body_edits(source, style.semantics()?.body(), &mut edits)?;
    }
    Ok(edits)
}

fn collect_style_body_edits(
    source: &str,
    body: &AttachedStyleBody,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    for member in body.members() {
        let AttachedStyleMember::Environment(environment) = member else {
            continue;
        };
        if let Some(edit) = canonical_environment_edit(source, environment) {
            edits.push(edit);
        }
        collect_style_body_edits(source, environment.body(), edits)?;
    }
    Ok(())
}

fn canonical_environment_edit(
    source: &str,
    environment: &AttachedStyleEnvironment,
) -> Option<TextEdit> {
    let condition = environment.condition();
    if condition.has_recovery() || condition.clauses().is_empty() {
        return None;
    }
    let mut clauses = condition.clauses().iter().collect::<Vec<_>>();
    clauses.sort_by_key(|clause| clause.field().value());
    let canonical = clauses
        .into_iter()
        .map(canonical_environment_clause)
        .collect::<Option<Vec<_>>>()?;
    let replacement = if canonical.len() == 1 {
        canonical[0].clone()
    } else {
        let indentation = line_indentation(source, environment.syntax().range().start());
        let clause_indentation = format!("{indentation}    ");
        let mut replacement = String::new();
        replacement.push('\n');
        for clause in canonical {
            replacement.push_str(&clause_indentation);
            replacement.push_str(&clause);
            replacement.push_str(",\n");
        }
        replacement.push_str(indentation);
        replacement
    };
    let start = condition.open_delimiter().range().end();
    let end = condition.close_delimiter().range().start();
    (source.get(start..end)? != replacement).then_some(TextEdit {
        start,
        end,
        replacement,
    })
}

fn canonical_environment_clause(clause: &AttachedStyleEnvironmentClause) -> Option<String> {
    use arcweft_lang_syntax::attachment::{
        StyleEnvironmentComparisonKind as Comparison, StyleEnvironmentFieldKind as Field,
    };

    let field = match clause.field().value()? {
        Field::ColorScheme => "color-scheme",
        Field::Contrast => "contrast",
        Field::ReducedMotion => "reduced-motion",
        Field::TextScale => "text-scale",
    };
    let comparison = match clause.comparison().value()? {
        Comparison::Equal => "==",
        Comparison::NotEqual => "!=",
        Comparison::Less => "<",
        Comparison::LessOrEqual => "<=",
        Comparison::Greater => ">",
        Comparison::GreaterOrEqual => ">=",
    };
    let AttachedStyleExpression::Authored(value) = clause.value() else {
        return None;
    };
    let value = match value.projection() {
        ExpressionProjection::Path => {
            let path = value.path()?;
            if path.has_recovery()
                || !matches!(path.root(), AttachedPathRoot::ImplicitCrate)
                || path.segments().len() != 1
            {
                return None;
            }
            path.segments()[0].source_text().to_ascii_lowercase()
        }
        ExpressionProjection::Literal(literal) => match literal.value() {
            SyntaxLiteralValue::Bool(value) => value.to_string(),
            SyntaxLiteralValue::Unit {
                value,
                unit: UnitNumberSuffix::Percent,
            } if value.exponent().is_none() && value.suffix().is_none() => {
                canonical_percentage(value.integral_digits(), value.fractional_digits())
            }
            _ => return None,
        },
        _ => return None,
    };
    Some(format!("{field} {comparison} {value}"))
}

fn canonical_percentage(integral: &str, fractional: Option<&str>) -> String {
    let integral = integral.trim_start_matches('0');
    let integral = if integral.is_empty() { "0" } else { integral };
    match fractional {
        None | Some("0") => format!("{integral}%"),
        Some(fractional) => format!("{integral}.{fractional}%"),
    }
}

fn line_indentation(source: &str, offset: usize) -> &str {
    let line_start = source[..offset]
        .rfind('\n')
        .map_or(0, |newline| newline + '\n'.len_utf8());
    let prefix = &source[line_start..offset];
    let indentation_end = prefix
        .find(|ch: char| !matches!(ch, ' ' | '\t'))
        .unwrap_or(prefix.len());
    &prefix[..indentation_end]
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RichTextCanonicalizationContext {
    text_proxy_types: BTreeSet<String>,
}

fn canonical_rich_text_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Result<Vec<TextEdit>, SyntaxAccessError> {
    let context = RichTextCanonicalizationContext {
        text_proxy_types: collect_text_proxy_type_names(parsed)?,
    };
    let mut edits = Vec::new();
    for item in parsed.items()? {
        match item {
            TypedItemNode::Flow(flow) => {
                let flow = flow.semantics()?;
                if let AttachedRequiredFlowBody::Present(body) = flow.body() {
                    visit_thread_flow_items(source, body.items(), &context, &mut edits)?;
                }
            }
            TypedItemNode::Function(function) => {
                let function = function.semantics()?;
                if let AttachedFunctionBody::Block { block, .. } = function.body() {
                    visit_value_block(source, block, &context, &mut edits)?;
                }
            }
            TypedItemNode::View(view) => {
                let view = view.semantics()?;
                if let Some(fragment) = view.body().fragment() {
                    for value in fragment.values() {
                        visit_expression(source, value, &context, &mut edits)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(edits)
}

fn collect_text_proxy_type_names(
    parsed: &ParsedSource,
) -> Result<BTreeSet<String>, SyntaxAccessError> {
    let mut names = BTreeSet::new();
    for item in parsed.items()? {
        let TypedItemNode::Struct(structure) = item else {
            continue;
        };
        let structure = structure.semantics()?;
        let is_proxy = structure.prefix().attributes().iter().any(|attribute| {
            attribute.issue().is_none()
                && matches!(attribute.path().root(), AttachedPathRoot::ImplicitCrate)
                && attribute.path().missing_name().is_none()
                && matches!(
                    attribute.path().segments(),
                    [segment]
                        if matches!(segment.source_text(), "text_proxy" | "rich_text_proxy")
                )
        });
        if is_proxy && let Some(name) = structure.name().value() {
            names.insert(name.as_str().to_owned());
        }
    }
    Ok(names)
}

fn visit_thread_flow_items(
    source: &str,
    items: &[AttachedThreadFlowItem],
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    for item in items {
        match item {
            AttachedThreadFlowItem::DialogueApplication(_) => {
                if let Some(expression) = item.dialogue_application() {
                    visit_expression(source, &expression, context, edits)?;
                }
            }
            AttachedThreadFlowItem::Statement(statement) => {
                visit_statement(source, statement, context, edits)?;
            }
            AttachedThreadFlowItem::If(conditional)
            | AttachedThreadFlowItem::IfLet(conditional) => {
                visit_if_statement(source, conditional, context, edits, true)?;
            }
            AttachedThreadFlowItem::While(while_statement) => {
                let statement = while_statement.semantics()?;
                visit_required_expression(source, statement.condition(), context, edits)?;
                visit_nested_thread_body(source, statement.body(), context, edits)?;
            }
            AttachedThreadFlowItem::WhileLet(while_statement) => {
                let statement = while_statement.semantics()?;
                visit_required_expression(source, statement.scrutinee(), context, edits)?;
                if let Some(guard) = statement.guard() {
                    visit_required_expression(source, guard, context, edits)?;
                }
                visit_nested_thread_body(source, statement.body(), context, edits)?;
            }
            AttachedThreadFlowItem::For(for_statement) => {
                let statement = for_statement.semantics()?;
                visit_required_expression(source, statement.source(), context, edits)?;
                visit_nested_thread_body(source, statement.body(), context, edits)?;
            }
            AttachedThreadFlowItem::SourceLocale(statement) => {
                visit_nested_thread_body(source, statement.semantics()?.body(), context, edits)?;
            }
            AttachedThreadFlowItem::Scope(statement) => {
                visit_nested_thread_body(source, statement.semantics()?.body(), context, edits)?;
            }
            AttachedThreadFlowItem::Select(statement) => {
                let statement = statement.semantics()?;
                match statement.form() {
                    arcweft_lang_syntax::attachment::AttachedSelectStatementForm::Operand(
                        operand,
                    ) => visit_required_expression(source, operand, context, edits)?,
                    arcweft_lang_syntax::attachment::AttachedSelectStatementForm::Branches(
                        branches,
                    ) => {
                        for branch in branches.branches() {
                            visit_nested_thread_body(source, branch.body(), context, edits)?;
                        }
                    }
                }
            }
            AttachedThreadFlowItem::Match(statement) => {
                visit_match_statement(source, statement, context, edits, true)?;
            }
            AttachedThreadFlowItem::Choice(_)
            | AttachedThreadFlowItem::Include(_)
            | AttachedThreadFlowItem::Error(_) => {}
        }
    }
    Ok(())
}

fn visit_nested_thread_body(
    source: &str,
    body: &AttachedRequiredNestedThreadFlowBody,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    if let AttachedRequiredNestedThreadFlowBody::Present(body) = body {
        visit_thread_flow_items(source, body.items(), context, edits)?;
    }
    Ok(())
}

fn visit_value_block(
    source: &str,
    block: &arcweft_lang_syntax::attachment::AstNode<BlockKind>,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    for statement in block.statements()? {
        visit_statement(source, &statement, context, edits)?;
    }
    if let Some(BlockTailNode::Expression(tail)) = block.optional_tail()? {
        visit_expression(source, &tail.semantic()?, context, edits)?;
    }
    Ok(())
}

fn visit_statement(
    source: &str,
    statement: &arcweft_lang_syntax::attachment::StatementNode,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    match statement.kind() {
        SyntaxKind::LetStatement => {
            if let Some(LetInitializerNode::Expression(value)) =
                statement.cast::<LetStatementKind>()?.initializer()?
            {
                visit_expression(source, &value.semantic()?, context, edits)?;
            }
        }
        SyntaxKind::ExpressionStatement => visit_expression(
            source,
            &statement
                .cast::<ExpressionStatementKind>()?
                .expression()?
                .semantic()?,
            context,
            edits,
        )?,
        SyntaxKind::AssignmentStatement => {
            let statement = statement.cast::<AssignmentStatementKind>()?;
            visit_required_expression(source, &statement.target()?, context, edits)?;
            visit_required_expression(source, &statement.value()?, context, edits)?;
        }
        SyntaxKind::LifetimeSetStatement => {
            let statement = statement.cast::<LifetimeSetStatementKind>()?;
            visit_required_expression(source, &statement.target()?, context, edits)?;
            visit_required_expression(source, &statement.value()?, context, edits)?;
        }
        SyntaxKind::ReturnStatement => visit_required_expression(
            source,
            &statement.cast::<ReturnStatementKind>()?.value()?,
            context,
            edits,
        )?,
        SyntaxKind::YieldStatement => visit_required_expression(
            source,
            &statement.cast::<YieldStatementKind>()?.expression()?,
            context,
            edits,
        )?,
        SyntaxKind::WaitStatement => visit_required_expression(
            source,
            &statement.cast::<WaitStatementKind>()?.target()?,
            context,
            edits,
        )?,
        SyntaxKind::CloseStatement => visit_required_expression(
            source,
            &statement.cast::<CloseStatementKind>()?.target()?,
            context,
            edits,
        )?,
        SyntaxKind::AssertionStatement => {
            for condition in statement.cast::<AssertionStatementKind>()?.conditions()? {
                visit_expression(source, &condition.semantic()?, context, edits)?;
            }
        }
        SyntaxKind::OutStatement => {
            let statement = statement.cast::<OutStatementKind>()?.semantics()?;
            visit_required_expression(source, statement.value(), context, edits)?;
        }
        SyntaxKind::GotoStatement => {
            let statement = statement.cast::<GotoStatementKind>()?.semantics()?;
            visit_required_expression(source, statement.target(), context, edits)?;
        }
        SyntaxKind::DeferStatement => {
            let statement = statement.cast::<DeferStatementKind>()?.semantics()?;
            visit_required_expression(source, statement.expression(), context, edits)?;
        }
        SyntaxKind::SignalStatement => {
            let statement = statement.cast::<SignalStatementKind>()?.semantics()?;
            visit_required_expression(source, statement.target(), context, edits)?;
            visit_required_expression(source, statement.value(), context, edits)?;
        }
        SyntaxKind::BreakStatement => {
            if let Some(value) = statement.cast::<BreakStatementKind>()?.semantics()?.value() {
                visit_expression(source, value, context, edits)?;
            }
        }
        SyntaxKind::IfStatement => visit_if_statement(
            source,
            &statement.cast::<IfStatementKind>()?,
            context,
            edits,
            false,
        )?,
        SyntaxKind::MatchStatement => visit_match_statement(
            source,
            &statement.cast::<MatchStatementKind>()?,
            context,
            edits,
            false,
        )?,
        _ => {}
    }
    Ok(())
}

fn visit_if_statement(
    source: &str,
    statement: &arcweft_lang_syntax::attachment::AstNode<IfStatementKind>,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
    thread_flow_body: bool,
) -> Result<(), SyntaxAccessError> {
    match statement.head()? {
        IfStatementHeadNode::Condition(condition) => {
            visit_expression(source, &condition.semantic()?, context, edits)?;
        }
        IfStatementHeadNode::Let {
            scrutinee, guard, ..
        } => {
            visit_expression(source, &scrutinee.semantic()?, context, edits)?;
            if let Some(guard) = guard {
                visit_expression(source, &guard.semantic()?, context, edits)?;
            }
        }
    }
    if thread_flow_body {
        let body = statement.then_branch()?.thread_flow_body()?;
        visit_thread_flow_items(source, body.items(), context, edits)?;
    } else {
        visit_value_block(source, &statement.then_branch()?, context, edits)?;
    }
    if let Some(otherwise) = statement.else_branch()? {
        match otherwise {
            IfStatementElseNode::Block(block) if thread_flow_body => {
                let body = block.thread_flow_body()?;
                visit_thread_flow_items(source, body.items(), context, edits)?;
            }
            IfStatementElseNode::Block(block) => {
                visit_value_block(source, &block, context, edits)?;
            }
            IfStatementElseNode::If(statement) => {
                visit_statement(source, &statement, context, edits)?;
            }
        }
    }
    Ok(())
}

fn visit_match_statement(
    source: &str,
    statement: &arcweft_lang_syntax::attachment::AstNode<MatchStatementKind>,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
    thread_flow_body: bool,
) -> Result<(), SyntaxAccessError> {
    if let MatchStatementExpressionNode::Expression(scrutinee) = statement.scrutinee()? {
        visit_expression(source, &scrutinee.semantic()?, context, edits)?;
    }
    let body = statement.body_or_missing()?;
    for arm in body.arms()? {
        if let Some(MatchStatementExpressionNode::Expression(guard)) = arm.guard()? {
            visit_expression(source, &guard.semantic()?, context, edits)?;
        }
        match arm.body()? {
            MatchStatementArmBodyNode::Expression(value) => {
                visit_expression(source, &value.semantic()?, context, edits)?;
            }
            MatchStatementArmBodyNode::Statement(statement) => {
                visit_statement(source, &statement, context, edits)?;
            }
            MatchStatementArmBodyNode::Block(block) if thread_flow_body => {
                let body = block.thread_flow_body()?;
                visit_thread_flow_items(source, body.items(), context, edits)?;
            }
            MatchStatementArmBodyNode::Block(block) => {
                visit_value_block(source, &block, context, edits)?;
            }
            MatchStatementArmBodyNode::Missing(_) => {}
        }
    }
    Ok(())
}

fn visit_required_expression(
    source: &str,
    expression: &RequiredStatementExpressionNode,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    if let RequiredStatementExpressionNode::Expression(expression) = expression {
        visit_expression(source, &expression.semantic()?, context, edits)?;
    }
    Ok(())
}

fn visit_expression(
    source: &str,
    expression: &AttachedExpressionNode,
    context: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    if let ExpressionProjection::DialogueContentApplication(application) = expression.projection()
        && let SyntaxDialogueContentProjection::Present(content) = application.content()
    {
        collect_dialogue_content_edits(source, expression, content, context, edits);
    }
    for child in expression.children() {
        if let Some(child) = child.authored_semantic()? {
            visit_expression(source, &child, context, edits)?;
        }
    }
    if let Some(arcweft_lang_syntax::attachment::AttachedAwaitBranchBody::Present(body)) =
        expression.await_branches()
    {
        for branch in body.branches() {
            visit_nested_thread_body(source, branch.body(), context, edits)?;
        }
    }
    for arm in expression.match_arms() {
        if let Some(guard) = arm.guard()
            && let Some(guard) = guard.authored_semantic()?
        {
            visit_expression(source, &guard, context, edits)?;
        }
        if let Some(value) = arm.value().authored_semantic()? {
            visit_expression(source, &value, context, edits)?;
        }
    }
    if let Some(block) = expression.block() {
        visit_value_block(source, block, context, edits)?;
    }
    if let Some(thread) = expression.thread()
        && let AttachedRequiredThreadExpressionBody::Present(body) = thread.statement_body()?
    {
        visit_thread_flow_items(source, body.items(), context, edits)?;
    }
    Ok(())
}

fn collect_dialogue_content_edits(
    source: &str,
    expression: &AttachedExpressionNode,
    content: &arcweft_lang_syntax::expressions::SyntaxDialogueContent,
    canonicalization: &RichTextCanonicalizationContext,
    edits: &mut Vec<TextEdit>,
) {
    for node in content.nodes() {
        let SyntaxDialogueNodeProjection::InferredStartTag { tag } = node else {
            continue;
        };
        let Some(projection) = content.tags().get(*tag as usize) else {
            continue;
        };
        let (proxy_type, family_name) = match projection.identity() {
            SyntaxRichTextTagIdentity::DotSelector(Ok(selector)) => {
                let selector = selector.as_str();
                let proxy_type =
                    inferred_text_proxy_type(selector, projection.arguments(), canonicalization);
                if proxy_type.is_none() {
                    // An unresolved marker has no schema-owned canonical family.
                    // Preserve it verbatim instead of inferring semantics from
                    // its spelling or the presence of raw attributes.
                    continue;
                }
                (proxy_type, "object")
            }
            SyntaxRichTextTagIdentity::Builtin(builtin) => {
                let family_name = match builtin {
                    SyntaxBuiltinRichTextTag::Style(_) => "style",
                    SyntaxBuiltinRichTextTag::Layout(_) => "layout",
                    SyntaxBuiltinRichTextTag::Transform(_) => "transform",
                    SyntaxBuiltinRichTextTag::Object(_) => "object",
                    SyntaxBuiltinRichTextTag::Fx(_) => "effect",
                    SyntaxBuiltinRichTextTag::Page
                    | SyntaxBuiltinRichTextTag::LineWait
                    | SyntaxBuiltinRichTextTag::HardBreak
                    | SyntaxBuiltinRichTextTag::TimedWait
                    | SyntaxBuiltinRichTextTag::Clear
                    | SyntaxBuiltinRichTextTag::Reset
                    | SyntaxBuiltinRichTextTag::Speed
                    | SyntaxBuiltinRichTextTag::Marker
                    | SyntaxBuiltinRichTextTag::DirectStyle(_)
                    | SyntaxBuiltinRichTextTag::HostEvent(_)
                    | SyntaxBuiltinRichTextTag::Conditional(_) => continue,
                };
                (None, family_name)
            }
            SyntaxRichTextTagIdentity::DotSelector(Err(_))
            | SyntaxRichTextTagIdentity::ProjectSymbol(_)
            | SyntaxRichTextTagIdentity::Invalid(_) => continue,
        };
        let tag_index = *tag;
        let Some(whole) = expression.component(ExpressionComponentRole::RichTextTag {
            tag: tag_index,
            part: SyntaxRichTextTagSourcePart::Whole,
        }) else {
            continue;
        };
        let Some(name) = expression.component(ExpressionComponentRole::RichTextTag {
            tag: tag_index,
            part: SyntaxRichTextTagSourcePart::Name,
        }) else {
            continue;
        };
        let Some(close) = expression.component(ExpressionComponentRole::RichTextTag {
            tag: tag_index,
            part: SyntaxRichTextTagSourcePart::CloseDelimiter,
        }) else {
            continue;
        };
        let Some(selector_source) = source.get(name.range().as_range()) else {
            continue;
        };
        let Some(argument_source) = source.get(name.range().end()..close.range().start()) else {
            continue;
        };
        let inserted_proxy_type = proxy_type.filter(|_| {
            !projection.arguments().iter().any(|argument| {
                matches!(
                    argument,
                    SyntaxRichTextArgumentProjection::Named { name: Ok(name), .. }
                        if matches!(name.as_str(), "type" | "struct" | "proxy")
                )
            })
        });
        let argument_source = if family_name == "mark" {
            ""
        } else {
            argument_source
        };
        let replacement = inserted_proxy_type.map_or_else(
            || format!("[{family_name} {selector_source}{argument_source}]"),
            |proxy| format!("[{family_name} {selector_source} type={proxy}{argument_source}]"),
        );
        push_if_changed(source, whole.range(), replacement, edits);

        let Some(end) = expression.component(ExpressionComponentRole::RichTextTag {
            tag: tag_index,
            part: SyntaxRichTextTagSourcePart::EndTag,
        }) else {
            continue;
        };
        let replacement = if family_name == "mark" {
            String::new()
        } else {
            format!("[/{family_name}]")
        };
        push_if_changed(source, end.range(), replacement, edits);
    }
}

fn inferred_text_proxy_type<'a>(
    selector: &'a str,
    arguments: &'a [SyntaxRichTextArgumentProjection],
    context: &'a RichTextCanonicalizationContext,
) -> Option<&'a str> {
    arguments
        .iter()
        .find_map(|argument| match argument {
            SyntaxRichTextArgumentProjection::Named {
                name: Ok(name),
                value,
            } if matches!(name.as_str(), "type" | "struct" | "proxy") => context
                .text_proxy_types
                .contains(value.decoded())
                .then_some(value.decoded()),
            _ => None,
        })
        .or_else(|| {
            context
                .text_proxy_types
                .contains(selector)
                .then_some(selector)
        })
}

fn push_if_changed(
    source: &str,
    range: SourceRange,
    replacement: String,
    edits: &mut Vec<TextEdit>,
) {
    if source
        .get(range.as_range())
        .is_some_and(|authored| authored != replacement)
    {
        edits.push(TextEdit {
            start: range.start(),
            end: range.end(),
            replacement,
        });
    }
}
