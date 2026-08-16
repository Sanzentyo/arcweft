//! Accepted-HIR projection for script tests and benches.

use arcweft_lang_hir::{
    expr::{
        HirCallArgument, HirCallCallee, HirCallExpr, HirCallValue, HirExprKind, HirNamedBlockExpr,
        HirNamedBlockName, HirSelectedMember, HirThreadFlowItem,
    },
    identity::{ExprId, ItemId, StmtId},
    item::{HirBenchItem, HirItemKind, HirTestItem, HirTestKind},
    leaf::{
        HirCharacterLiteral, HirFloatLiteral, HirIdRef, HirIdRefValue, HirIntegerLiteral,
        HirLiteral, HirPath, HirPathRoot, HirPathSegment, HirStringLiteral,
    },
    module::HirModule,
    project::HirProject,
    source_index::{
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
        HirTestBenchSourceRole,
    },
    stmt::{HirAssertionMode, HirContextualStmtBody, HirStmtKind},
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use serde::{Deserialize, Serialize};

/// Tool-facing manifest of script-level tests and benches.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptTestManifest {
    pub tests: Vec<ScriptTest>,
    pub benches: Vec<ScriptBench>,
}

/// One top-level `test @test.id kind { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptTest {
    pub id: String,
    pub kind: String,
    pub steps: Vec<ScriptStep>,
    pub source: ManifestSpan,
}

/// One typed row inside a script test body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptStep {
    pub command: ScriptCommand,
    /// Diagnostic display text only. Consumers must not reconstruct semantics
    /// from this field.
    pub text: String,
    pub source: ManifestSpan,
}

/// One top-level `bench @bench.id { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptBench {
    pub id: String,
    pub sections: Vec<BenchSection>,
    pub source: ManifestSpan,
}

/// One typed top-level section inside a bench body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchSection {
    pub name: String,
    pub body: Vec<ScriptCommand>,
    /// Diagnostic display text only. Consumers must not reconstruct semantics
    /// from this field.
    pub text: String,
    pub source: ManifestSpan,
}

/// Closed command inventory consumed by the script test/bench runtime.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScriptCommand {
    Goto {
        target: String,
    },
    Expectation {
        expectation: ScriptExpectation,
    },
    Pure {
        helper: String,
    },
    Scope {
        name: Option<String>,
        body: Vec<Self>,
    },
    Other {
        name: String,
    },
}

/// Runtime expectation admitted from one typed `expect.*` Call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScriptExpectation {
    NoAssertionFailures,
    Signal {
        target: String,
        expected: String,
    },
    Log {
        level: String,
        contains: String,
    },
    File {
        path: ScriptVirtualPath,
        equals: String,
    },
    Unsupported {
        method: Option<String>,
    },
}

/// Typed virtual path accepted by `expect.file`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptVirtualPath {
    pub root: ScriptVirtualPathRoot,
    pub relative: String,
}

impl ScriptVirtualPath {
    /// Returns the runtime-host virtual path label.
    pub fn runtime_label(&self) -> String {
        format!("{}:{}", self.root.keyword(), self.relative)
    }
}

/// Closed runtime-host virtual path root.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptVirtualPathRoot {
    Save,
    Asset,
    Temp,
    Export,
}

impl ScriptVirtualPathRoot {
    const fn keyword(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Asset => "asset",
            Self::Temp => "temp",
            Self::Export => "export",
        }
    }
}

/// Stable byte span bound to one exact accepted source revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestSpan {
    pub document: SourceDocumentIdentity,
    pub start: usize,
    pub end: usize,
}

/// Extracts executable script tests and benches from the accepted final HIR.
///
/// Recovered declarations are deliberately absent: a manifest is an
/// executable inventory, not a second tooling view of poisoned syntax.
#[must_use]
pub fn collect_script_tests(project: &HirProject) -> ScriptTestManifest {
    let mut manifest = ScriptTestManifest::default();
    for item_ref in project.view().items() {
        let module = item_ref.module();
        match item_ref.item().kind() {
            HirItemKind::Test(item) => {
                manifest
                    .tests
                    .extend(script_test(module, item_ref.id(), item));
            }
            HirItemKind::Bench(item) => {
                manifest
                    .benches
                    .extend(script_bench(module, item_ref.id(), item));
            }
            _ => {}
        }
    }
    manifest
}

fn script_test(module: &HirModule, owner: ItemId, item: &HirTestItem) -> Option<ScriptTest> {
    Some(ScriptTest {
        id: id_ref_label(item.id().as_resolved()?, "test"),
        kind: test_kind_label(item.kind())?.to_owned(),
        steps: body_commands(module, item.body()),
        source: item_source(module, owner)?,
    })
}

fn script_bench(module: &HirModule, owner: ItemId, item: &HirBenchItem) -> Option<ScriptBench> {
    Some(ScriptBench {
        id: id_ref_label(item.id().as_resolved()?, "bench"),
        sections: bench_sections(module, item.body()),
        source: item_source(module, owner)?,
    })
}

fn body_commands(module: &HirModule, body: &[StmtId]) -> Vec<ScriptStep> {
    body.iter()
        .filter_map(|owner| {
            let statement = module.resolve_stmt(*owner).ok()?;
            let (source, text) = statement_source(module, *owner)?;
            Some(ScriptStep {
                command: script_command(module, statement.kind()),
                text,
                source,
            })
        })
        .collect()
}

fn bench_sections(module: &HirModule, body: &[StmtId]) -> Vec<BenchSection> {
    body.iter()
        .filter_map(|owner| {
            let statement = module.resolve_stmt(*owner).ok()?;
            let (source, text) = statement_source(module, *owner)?;
            let name = statement_name(module, statement.kind());
            let commands = match statement.kind() {
                HirStmtKind::Scope(scope) => contextual_commands(module, scope.body()),
                HirStmtKind::Expression { expression }
                    if named_block(module, *expression).is_some() =>
                {
                    named_block_commands(
                        module,
                        named_block(module, *expression)
                            .expect("guard establishes one typed named block"),
                    )
                }
                statement => vec![script_command(module, statement)],
            };
            Some(BenchSection {
                name,
                body: commands,
                text,
                source,
            })
        })
        .collect()
}

fn contextual_commands(module: &HirModule, body: &HirContextualStmtBody) -> Vec<ScriptCommand> {
    let statements = match body {
        HirContextualStmtBody::Ordinary { statements, .. } => statements.to_vec(),
        HirContextualStmtBody::Thread(body) => body
            .items()
            .iter()
            .filter_map(thread_item_statement)
            .collect(),
    };
    statements
        .into_iter()
        .filter_map(|owner| {
            let statement = module.resolve_stmt(owner).ok()?;
            Some(script_command(module, statement.kind()))
        })
        .collect()
}

fn thread_item_statement(item: &HirThreadFlowItem) -> Option<StmtId> {
    match item {
        HirThreadFlowItem::DialogueApplication(_) => None,
        HirThreadFlowItem::Statement(owner)
        | HirThreadFlowItem::Choice(owner)
        | HirThreadFlowItem::If(owner)
        | HirThreadFlowItem::IfLet(owner)
        | HirThreadFlowItem::Match(owner)
        | HirThreadFlowItem::Loop(owner)
        | HirThreadFlowItem::While(owner)
        | HirThreadFlowItem::WhileLet(owner)
        | HirThreadFlowItem::For(owner)
        | HirThreadFlowItem::Select(owner)
        | HirThreadFlowItem::SourceLocale(owner)
        | HirThreadFlowItem::Scope(owner)
        | HirThreadFlowItem::Include(owner)
        | HirThreadFlowItem::Error(owner) => Some(*owner),
    }
}

fn script_command(module: &HirModule, statement: &HirStmtKind) -> ScriptCommand {
    match statement {
        HirStmtKind::Goto { target } => goto_target(module, *target).map_or_else(
            || ScriptCommand::Other {
                name: "goto".to_owned(),
            },
            |target| ScriptCommand::Goto { target },
        ),
        HirStmtKind::Expression { expression } | HirStmtKind::ProofCall { call: expression } => {
            if let Some(block) = named_block(module, *expression) {
                return ScriptCommand::Scope {
                    name: named_block_name(block),
                    body: named_block_commands(module, block),
                };
            }
            script_expression_command(module, *expression)
        }
        HirStmtKind::Scope(scope) => ScriptCommand::Scope {
            name: scope.name().map(|name| name.as_str().to_owned()),
            body: contextual_commands(module, scope.body()),
        },
        _ => ScriptCommand::Other {
            name: statement_name(module, statement),
        },
    }
}

fn script_expression_command(module: &HirModule, expression: ExprId) -> ScriptCommand {
    let Some(call) = expression_call(module, expression) else {
        return ScriptCommand::Other {
            name: expression_label(module, expression).unwrap_or_else(|| "expression".to_owned()),
        };
    };
    if is_selected_call(module, call, "expect").is_some() {
        return ScriptCommand::Expectation {
            expectation: script_expectation(module, call),
        };
    }
    if let Some(helper) = pure_helper(module, call) {
        return ScriptCommand::Pure { helper };
    }
    ScriptCommand::Other {
        name: call_label(module, call.callee()).unwrap_or_else(|| "expression".to_owned()),
    }
}

fn named_block(module: &HirModule, owner: ExprId) -> Option<&HirNamedBlockExpr> {
    match module.resolve_expr(owner).ok()?.kind() {
        HirExprKind::NamedBlock(block) => Some(block),
        _ => None,
    }
}

fn named_block_name(block: &HirNamedBlockExpr) -> Option<String> {
    match block.name() {
        HirNamedBlockName::Resolved(name) => Some(name.as_str().to_owned()),
        HirNamedBlockName::InvalidPresent(_) => None,
    }
}

fn named_block_commands(module: &HirModule, block: &HirNamedBlockExpr) -> Vec<ScriptCommand> {
    let mut commands = block
        .statements()
        .iter()
        .filter_map(|owner| {
            module
                .resolve_stmt(*owner)
                .ok()
                .map(|statement| script_command(module, statement.kind()))
        })
        .collect::<Vec<_>>();
    if !matches!(
        module
            .resolve_expr(block.tail())
            .map(arcweft_lang_hir::expr::HirExpr::kind),
        Ok(HirExprKind::Unit)
    ) {
        commands.push(script_expression_command(module, block.tail()));
    }
    commands
}

fn script_expectation(module: &HirModule, call: &HirCallExpr) -> ScriptExpectation {
    let Some(method) = is_selected_call(module, call, "expect") else {
        return ScriptExpectation::Unsupported { method: None };
    };
    match method.as_str() {
        "no_assertion_failures" if call.arguments().is_empty() => {
            ScriptExpectation::NoAssertionFailures
        }
        "signal" => {
            expectation_signal(module, call).unwrap_or_else(|| ScriptExpectation::Unsupported {
                method: Some(method.clone()),
            })
        }
        "log" => expectation_log(module, call).unwrap_or_else(|| ScriptExpectation::Unsupported {
            method: Some(method.clone()),
        }),
        "file" => {
            expectation_file(module, call).unwrap_or_else(|| ScriptExpectation::Unsupported {
                method: Some(method.clone()),
            })
        }
        _ => ScriptExpectation::Unsupported {
            method: Some(method),
        },
    }
}

fn expectation_signal(module: &HirModule, call: &HirCallExpr) -> Option<ScriptExpectation> {
    let [target, expected] = call.arguments() else {
        return None;
    };
    Some(ScriptExpectation::Signal {
        target: manifest_value_label(module, present_argument(target)?)?,
        expected: manifest_value_label(module, present_argument(expected)?)?,
    })
}

fn expectation_log(module: &HirModule, call: &HirCallExpr) -> Option<ScriptExpectation> {
    let [level, contains] = call.arguments() else {
        return None;
    };
    let level = match module.resolve_expr(present_argument(level)?).ok()?.kind() {
        HirExprKind::ShortVariant(name) => name.as_resolved()?.as_str().to_owned(),
        HirExprKind::Select(select)
            if expression_label(module, select.target()).as_deref() == Some("log") =>
        {
            let HirSelectedMember::Name(member) = select.member() else {
                return None;
            };
            member.as_str().to_owned()
        }
        _ => return None,
    };
    if contains.resolved_name()?.as_str() != "contains" {
        return None;
    }
    Some(ScriptExpectation::Log {
        level,
        contains: string_value(module, present_argument(contains)?)?,
    })
}

fn expectation_file(module: &HirModule, call: &HirCallExpr) -> Option<ScriptExpectation> {
    let [path, equals] = call.arguments() else {
        return None;
    };
    if equals.resolved_name()?.as_str() != "equals" {
        return None;
    }
    Some(ScriptExpectation::File {
        path: virtual_path(module, present_argument(path)?)?,
        equals: string_value(module, present_argument(equals)?)?,
    })
}

fn virtual_path(module: &HirModule, owner: ExprId) -> Option<ScriptVirtualPath> {
    let call = expression_call(module, owner)?;
    let method = is_selected_call(module, call, "path")?;
    let root = match method.as_str() {
        "save" => ScriptVirtualPathRoot::Save,
        "asset" => ScriptVirtualPathRoot::Asset,
        "temp" => ScriptVirtualPathRoot::Temp,
        "export" => ScriptVirtualPathRoot::Export,
        _ => return None,
    };
    let [relative] = call.arguments() else {
        return None;
    };
    Some(ScriptVirtualPath {
        root,
        relative: string_value(module, present_argument(relative)?)?,
    })
}

fn pure_helper(module: &HirModule, call: &HirCallExpr) -> Option<String> {
    if call_label(module, call.callee()).as_deref() != Some("pure") {
        return None;
    }
    let [helper] = call.arguments() else {
        return None;
    };
    expression_label(module, present_argument(helper)?)
}

fn manifest_value_label(module: &HirModule, owner: ExprId) -> Option<String> {
    match module.resolve_expr(owner).ok()?.kind() {
        HirExprKind::EntityReference(reference) => id_ref_value_label(reference),
        HirExprKind::Path(path) => Some(path_label(path.as_resolved()?)),
        HirExprKind::ShortVariant(name) => Some(format!(".{}", name.as_resolved()?.as_str())),
        HirExprKind::Select(_) => expression_label(module, owner),
        HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(value))) => {
            Some(value.to_string())
        }
        HirExprKind::Literal(HirLiteral::Character(HirCharacterLiteral::Value(value))) => {
            Some(value.to_string())
        }
        HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
            magnitude, ..
        })) => Some(magnitude.to_decimal_string()),
        HirExprKind::Literal(HirLiteral::Float(HirFloatLiteral::Value { decimal, .. })) => {
            Some(decimal.to_decimal_string())
        }
        HirExprKind::Literal(HirLiteral::Boolean(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn string_value(module: &HirModule, owner: ExprId) -> Option<String> {
    match module.resolve_expr(owner).ok()?.kind() {
        HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(value))) => {
            Some(value.to_string())
        }
        _ => None,
    }
}

fn present_argument(argument: &HirCallArgument) -> Option<ExprId> {
    match argument.value_state() {
        HirCallValue::Present { value } => Some(*value),
        HirCallValue::Missing { .. } => None,
    }
}

fn expression_call(module: &HirModule, owner: ExprId) -> Option<&HirCallExpr> {
    match module.resolve_expr(owner).ok()?.kind() {
        HirExprKind::Call(call) => Some(call),
        _ => None,
    }
}

fn is_selected_call(module: &HirModule, call: &HirCallExpr, target: &str) -> Option<String> {
    match call.callee() {
        HirCallCallee::Value { value } => {
            let HirExprKind::Select(select) = module.resolve_expr(*value).ok()?.kind() else {
                return None;
            };
            if expression_label(module, select.target()).as_deref() != Some(target) {
                return None;
            }
            let HirSelectedMember::Name(member) = select.member() else {
                return None;
            };
            Some(member.as_str().to_owned())
        }
        HirCallCallee::UnresolvedDot {
            value_receiver,
            member,
            ..
        } if expression_label(module, *value_receiver).as_deref() == Some(target) => {
            Some(member.resolved()?.as_str().to_owned())
        }
        HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => None,
    }
}

fn goto_target(module: &HirModule, owner: ExprId) -> Option<String> {
    let HirExprKind::EntityReference(reference) = module.resolve_expr(owner).ok()?.kind() else {
        return None;
    };
    match reference.as_resolved()? {
        HirIdRef::Absolute(reference) => Some(reference.as_str().to_owned()),
        HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_) => None,
    }
}

fn id_ref_value_label(reference: &HirIdRefValue) -> Option<String> {
    match reference.as_resolved()? {
        HirIdRef::Absolute(reference) => Some(format!("@{}", reference.as_str())),
        HirIdRef::Relative(relative) => Some(format!(
            "@{}{}",
            ".".repeat(relative.parent_depth().saturating_add(1)),
            relative.suffix().as_str()
        )),
        HirIdRef::FamilyRelative(relative) => Some(format!(
            "@{}:{}{}",
            relative.family().as_str(),
            ".".repeat(relative.relative().parent_depth().saturating_add(1)),
            relative.relative().suffix().as_str()
        )),
    }
}

fn id_ref_label(id: &HirIdRef, default_family: &str) -> String {
    match id {
        HirIdRef::Absolute(entity) => entity.as_str().to_owned(),
        HirIdRef::Relative(relative) => format!("{default_family}.{}", relative.suffix().as_str()),
        HirIdRef::FamilyRelative(relative) => format!(
            "{}.{}",
            relative.family().as_str(),
            relative.relative().suffix().as_str()
        ),
    }
}

fn test_kind_label(kind: &HirTestKind) -> Option<&str> {
    match kind {
        HirTestKind::Scenario => Some("scenario"),
        HirTestKind::Visual => Some("visual"),
        HirTestKind::Audio => Some("audio"),
        HirTestKind::Fixture => Some("fixture"),
        HirTestKind::Custom(name) => Some(name.as_str()),
        HirTestKind::Recovered(_) => None,
    }
}

fn item_source(module: &HirModule, owner: ItemId) -> Option<ManifestSpan> {
    exact_span(
        module,
        HirSourceQuery::Item {
            owner,
            role: HirItemSourceRole::TestBench(HirTestBenchSourceRole::Whole),
        },
    )
    .map(manifest_span)
}

fn statement_source(module: &HirModule, owner: StmtId) -> Option<(ManifestSpan, String)> {
    let source = exact_span(
        module,
        HirSourceQuery::Stmt {
            owner,
            role: HirStmtSourceRole::Whole,
        },
    )?;
    let document = module.provenance().document();
    let text = document.text().get(source.range().as_range())?.trim();
    (!text.is_empty()).then(|| (manifest_span(source), text.to_owned()))
}

fn exact_span(module: &HirModule, query: HirSourceQuery) -> Option<&SourceSpan> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .ok()?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(source)) => Some(source),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    }
}

fn manifest_span(source: &SourceSpan) -> ManifestSpan {
    let range = source.range();
    ManifestSpan {
        document: source.source().clone(),
        start: range.start(),
        end: range.end(),
    }
}

fn statement_name(module: &HirModule, statement: &HirStmtKind) -> String {
    let fixed = match statement {
        HirStmtKind::Assertion { mode, .. } => {
            return match mode {
                HirAssertionMode::Resolved(mode) => format!("assert.{}", mode.keyword()),
                HirAssertionMode::Recovered => "assert".to_owned(),
            };
        }
        HirStmtKind::Let { .. }
        | HirStmtKind::LetElse { .. }
        | HirStmtKind::LetChoice { .. }
        | HirStmtKind::LetScope { .. }
        | HirStmtKind::LetLoop { .. }
        | HirStmtKind::LetActionReceive { .. } => "let",
        HirStmtKind::Assign { target, .. } => {
            return expression_label(module, *target).unwrap_or_else(|| "assign".to_owned());
        }
        HirStmtKind::Return { .. } => "return",
        HirStmtKind::Out { .. } => "out",
        HirStmtKind::Goto { .. } => "goto",
        HirStmtKind::DeferBlock { .. } | HirStmtKind::Defer { .. } => "defer",
        HirStmtKind::Yield { .. } => "yield",
        HirStmtKind::Signal { .. } => "signal",
        HirStmtKind::LifetimeSet { .. } => "lifetime",
        HirStmtKind::Wait { .. } => "wait",
        HirStmtKind::On { .. } => "on",
        HirStmtKind::UnsafeLifetime { .. } => "unsafe",
        HirStmtKind::Choice { .. } => "choice",
        HirStmtKind::If(_) | HirStmtKind::IfLet(_) => "if",
        HirStmtKind::Match(_) => "match",
        HirStmtKind::Loop(_) => "loop",
        HirStmtKind::While(_) | HirStmtKind::WhileLet(_) => "while",
        HirStmtKind::For(_) => "for",
        HirStmtKind::Close { .. } => "close",
        HirStmtKind::Select(_) => "select",
        HirStmtKind::SourceLocale(_) => "source.locale",
        HirStmtKind::Scope(scope) => {
            return scope
                .name()
                .map_or_else(|| "scope".to_owned(), |name| name.as_str().to_owned());
        }
        HirStmtKind::Include(_) => "include",
        HirStmtKind::Break { .. } => "break",
        HirStmtKind::Continue { .. } => "continue",
        HirStmtKind::Expression { expression } | HirStmtKind::ProofCall { call: expression } => {
            return expression_label(module, *expression)
                .unwrap_or_else(|| "expression".to_owned());
        }
        HirStmtKind::Error => "error",
    };
    fixed.to_owned()
}

fn expression_label(module: &HirModule, owner: ExprId) -> Option<String> {
    match module.resolve_expr(owner).ok()?.kind() {
        HirExprKind::Path(path) => path.as_resolved().map(path_label),
        HirExprKind::Select(select) => {
            let mut label = expression_label(module, select.target())?;
            let HirSelectedMember::Name(member) = select.member() else {
                return None;
            };
            label.push('.');
            label.push_str(member.as_str());
            Some(label)
        }
        HirExprKind::Call(call) => call_label(module, call.callee()),
        HirExprKind::Thread(_) => Some("thread".to_owned()),
        HirExprKind::NamedBlock(block) => named_block_name(block),
        _ => None,
    }
}

fn call_label(module: &HirModule, callee: &HirCallCallee) -> Option<String> {
    match callee {
        HirCallCallee::Value { value } => expression_label(module, *value),
        HirCallCallee::UnresolvedDot {
            value_receiver,
            member,
            ..
        } => {
            let mut label = expression_label(module, *value_receiver)?;
            label.push('.');
            label.push_str(member.resolved()?.as_str());
            Some(label)
        }
        HirCallCallee::Associated { member, .. } => Some(member.resolved()?.as_str().to_owned()),
    }
}

fn path_label(path: &HirPath) -> String {
    let mut segments = Vec::new();
    match path.root() {
        HirPathRoot::ImplicitCrate => {}
        HirPathRoot::Crate => segments.push("crate".to_owned()),
        HirPathRoot::SelfModule => segments.push("self".to_owned()),
        HirPathRoot::Super { depth } => {
            segments.extend((0..depth).map(|_| "super".to_owned()));
        }
    }
    segments.extend(path.segments().iter().map(|segment| match segment {
        HirPathSegment::Identifier(name) => name.as_str().to_owned(),
        HirPathSegment::ProjectSymbol(name) => name.as_str().to_owned(),
    }));
    segments.join(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::{
        database::HirDatabase,
        lowering::{HirModuleKey, LoweringRequest},
        project::{HirProjectBuilder, HirProjectModule},
        proof_return::HirProofReturnSemanticFactSet,
        symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId},
    };
    use arcweft_lang_syntax::{
        ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
    };
    use arcweft_source::{
        SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
    };
    use std::sync::Arc;

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the test verifies one typed script manifest from bound syntax through accepted HIR"
    )]
    fn collects_typed_script_test_and_bench_manifest() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://arcweft-test/script-manifest.arcw")
                    .expect("script manifest fixture source ID"),
                SourceName::path("arcweft-test/script-manifest.arcw"),
                r#"
test @test.opening scenario {
    goto @flow.opening
    expect.signal(@signal.current_flow, true)
    expect.no_assertion_failures()
}

bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 10 { pure(opening_choices) }
    report { cpu_time }
}
"#,
            )
            .expect("script manifest fixture source document"),
        );
        let mut syntax = SyntaxDatabase::try_new().unwrap();
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                Arc::clone(&document),
                ParseOptions::default(),
            )
            .expect("attached source parses");
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let package = CallablePackageId::try_new("arcweft-test-manifest-tests").unwrap();
        let path = CanonicalModulePath::crate_root();
        let key = HirModuleKey::new(package.clone(), path.clone(), document.identity().clone());
        let mut database = HirDatabase::try_new().unwrap();
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            document.identity().id().clone(),
            "arcweft-test-manifest-tests",
        )
        .unwrap();
        let revision = ProjectSymbolRevision::try_for_documents([document.identity()]).unwrap();
        let transaction = database
            .stage_proof_return_project(
                [LoweringRequest::try_new(key, &parsed).unwrap()],
                world,
                revision,
                [document.identity()],
                arcweft_lang_hir::lowering::HirLoweringControl::new(),
            )
            .expect("final HIR project stages");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("script manifest fixture has no authored Proof return headers");
        let mut outputs = transaction
            .publish_with_semantic_facts(&mut database, facts)
            .expect("final HIR project publishes");
        let module = outputs.pop().expect("one fixture module").into_module();
        assert!(outputs.is_empty());
        let bound = HirProjectModule::try_new(
            &database,
            &package,
            &path,
            document.identity(),
            Arc::clone(&module),
        )
        .unwrap();
        let mut builder = HirProjectBuilder::new(&database, package);
        builder.insert_module(bound).unwrap();
        let project = builder.finish().unwrap();
        let manifest = collect_script_tests(&project);

        assert_eq!(manifest.tests[0].id, "test.opening");
        assert_eq!(manifest.tests[0].kind, "scenario");
        assert_eq!(
            manifest.tests[0].steps[0].command,
            ScriptCommand::Goto {
                target: "flow.opening".to_owned()
            }
        );
        assert_eq!(
            manifest.tests[0].steps[1].command,
            ScriptCommand::Expectation {
                expectation: ScriptExpectation::Signal {
                    target: "@signal.current_flow".to_owned(),
                    expected: "true".to_owned(),
                }
            }
        );
        assert_eq!(
            manifest.tests[0].steps[2].command,
            ScriptCommand::Expectation {
                expectation: ScriptExpectation::NoAssertionFailures
            }
        );
        assert_eq!(manifest.benches[0].sections[1].name, "measure");
        assert_eq!(
            manifest.benches[0].sections[1].body,
            [ScriptCommand::Pure {
                helper: "opening_choices".to_owned()
            }]
        );
        assert_eq!(
            manifest.benches[0].sections[1].source.document,
            document.identity().clone()
        );
    }
}
