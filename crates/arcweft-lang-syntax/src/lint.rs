use crate::ast::choice::{ChoiceAction, ChoiceItem};
use crate::ast::common::TextRange;
use crate::ast::flow::FlowItem;
use crate::ast::ids::{FamilyRelativeEntityRef, IdRef, RelativeId, RelativeIdSpelling};
use crate::ast::items::{Attribute, Item, TypedSyntaxTree};
use crate::ast::source::SourceItem;
use arcweft_source::{
    Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity, DiagnosticSuggestion,
    SourceEdit, SourceName, SourceRange, SourceSpan,
};

/// Syntax-level lint emitted before full name resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLint {
    code: SyntaxLintCode,
    message: String,
    range: TextRange,
    suggestions: Vec<SyntaxLintSuggestion>,
}

/// Stable categories for editor and CLI filtering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxLintCode {
    DeepDotRunRelativeId,
    FlowIdModuleMismatch,
    RedundantDeclIdentity,
    DeclBindingMismatch,
    ExplicitDeclId,
    GeneratedSurfaceForm,
}

/// Source edit attached to a syntax lint before the source name is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLintEdit {
    range: TextRange,
    replacement: String,
}

/// Concrete syntax lint suggestion that can become a terminal patch, LSP fix, or Agent action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLintSuggestion {
    message: String,
    edits: Vec<SyntaxLintEdit>,
    applicability: DiagnosticApplicability,
}

impl SyntaxLintCode {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::DeepDotRunRelativeId => "AWF0001",
            Self::FlowIdModuleMismatch => "AWF0002",
            Self::RedundantDeclIdentity => "AWF0101",
            Self::DeclBindingMismatch => "AWF0102",
            Self::ExplicitDeclId => "AWF0103",
            Self::GeneratedSurfaceForm => "AWF0104",
        }
    }

    pub const fn domain_name(self) -> &'static str {
        match self {
            Self::DeepDotRunRelativeId => "id::deep_dot_run",
            Self::FlowIdModuleMismatch => "id::flow_module_mismatch",
            Self::RedundantDeclIdentity => "style::redundant_decl_identity",
            Self::DeclBindingMismatch => "identity::decl_binding_mismatch",
            Self::ExplicitDeclId => "style::explicit_decl_id",
            Self::GeneratedSurfaceForm => "style::generated_surface_form",
        }
    }

    pub const fn default_severity(self) -> SyntaxLintSeverity {
        match self {
            Self::DeclBindingMismatch => SyntaxLintSeverity::Error,
            Self::DeepDotRunRelativeId
            | Self::FlowIdModuleMismatch
            | Self::RedundantDeclIdentity => SyntaxLintSeverity::Warning,
            Self::GeneratedSurfaceForm => SyntaxLintSeverity::Information,
            Self::ExplicitDeclId => SyntaxLintSeverity::Hint,
        }
    }
}

/// Default severity for a syntax lint before user lint-level overrides.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxLintSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl SyntaxLintSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "info",
            Self::Hint => "hint",
        }
    }

    pub const fn diagnostic_severity(self) -> DiagnosticSeverity {
        match self {
            Self::Error => DiagnosticSeverity::Error,
            Self::Warning => DiagnosticSeverity::Warning,
            Self::Information => DiagnosticSeverity::Info,
            Self::Hint => DiagnosticSeverity::Hint,
        }
    }
}

/// Lints ID policy choices that are parseable but discouraged.
pub fn lint_id_policy(tree: &TypedSyntaxTree) -> Vec<SyntaxLint> {
    let mut lints = Vec::new();
    for item in tree.items() {
        lint_item_ids(item, tree, &mut lints);
    }
    lints
}

fn lint_item_ids(item: &Item, tree: &TypedSyntaxTree, lints: &mut Vec<SyntaxLint>) {
    let source_attrs = tree.attrs();
    match item {
        Item::Flow(flow) => {
            if flow.has_explicit_name()
                && let (Some(id), Some(name)) = (flow.id(), flow.name())
            {
                lint_decl_identity(
                    "flow",
                    id.body(),
                    name,
                    *id.range(),
                    flow.attrs(),
                    source_attrs,
                    lints,
                );
            } else if let Some(id) = flow.id() {
                let name = flow
                    .name()
                    .or_else(|| id.body().rsplit('.').next())
                    .unwrap_or("flow");
                lint_explicit_decl_id(
                    "flow",
                    id.body(),
                    name,
                    *id.range(),
                    flow.attrs(),
                    source_attrs,
                    lints,
                );
            }
            if let (Some(module), Some(id)) = (tree.module(), flow.id()) {
                let module_tail = module.path().rsplit("::").next();
                let id_tail = id.body().rsplit('.').next();
                if module_tail != id_tail
                    && !allows_lint(
                        flow.attrs(),
                        source_attrs,
                        SyntaxLintCode::FlowIdModuleMismatch,
                    )
                {
                    lints.push(SyntaxLint::new(
                        SyntaxLintCode::FlowIdModuleMismatch,
                        format!(
                            "flow id `{}` does not follow module tail `{}`",
                            id.body(),
                            module_tail.unwrap_or_default()
                        ),
                        *id.range(),
                    ));
                }
            }
            for item in flow.body() {
                lint_flow_item_ids(item, lints);
            }
        }
        Item::EntityDecl(item) => {
            if let Some(name) = item.surface_alias().or_else(|| item.name()) {
                lint_decl_identity(
                    item.kind().keyword(),
                    item.id().body(),
                    name,
                    *item.id().range(),
                    item.attrs(),
                    source_attrs,
                    lints,
                );
            } else if let Some(name) = item.id().body().rsplit('.').next() {
                lint_explicit_decl_id(
                    item.kind().keyword(),
                    item.id().body(),
                    name,
                    *item.id().range(),
                    item.attrs(),
                    source_attrs,
                    lints,
                );
            }
        }
        Item::Source(source) => lint_source_identity(source, source_attrs, lints),
        Item::FlowItem(item) => lint_flow_item_ids(item, lints),
        _ => {}
    }
}

fn lint_source_identity(
    source: &SourceItem,
    source_attrs: &[Attribute],
    lints: &mut Vec<SyntaxLint>,
) {
    match (source.id(), source.name()) {
        (Some(id), Some(name)) => {
            lint_decl_identity(
                "source",
                id.body(),
                name,
                *id.range(),
                source.attrs(),
                source_attrs,
                lints,
            );
        }
        (Some(id), None) => {
            if let Some(name) = id.body().rsplit('.').next() {
                lint_explicit_decl_id(
                    "source",
                    id.body(),
                    name,
                    *id.range(),
                    source.attrs(),
                    source_attrs,
                    lints,
                );
            }
        }
        (None, _) => {}
    }
}

fn lint_decl_identity(
    kind: &str,
    id: &str,
    name: &str,
    range: TextRange,
    attrs: &[Attribute],
    source_attrs: &[Attribute],
    lints: &mut Vec<SyntaxLint>,
) {
    let Some(id_tail) = id.rsplit('.').next() else {
        return;
    };
    if id_tail == name {
        if is_generated(attrs, source_attrs) {
            lint_generated_surface_form(kind, id, name, range, attrs, source_attrs, lints);
            return;
        }
        if allows_lint(attrs, source_attrs, SyntaxLintCode::RedundantDeclIdentity) {
            return;
        }
        lints.push(SyntaxLint::new(
            SyntaxLintCode::RedundantDeclIdentity,
            format!("`{kind} @{id} {name}` repeats the same declaration identity twice"),
            range,
        ));
    } else {
        lints.push(SyntaxLint::new(
            SyntaxLintCode::DeclBindingMismatch,
            format!(
                "`{kind} @{id} {name}` binds declaration id `{id}` to mismatched name `{name}`"
            ),
            range,
        ));
    }
}

fn lint_explicit_decl_id(
    kind: &str,
    id: &str,
    name: &str,
    range: TextRange,
    attrs: &[Attribute],
    source_attrs: &[Attribute],
    lints: &mut Vec<SyntaxLint>,
) {
    if allows_lint(attrs, source_attrs, SyntaxLintCode::ExplicitDeclId) {
        return;
    }
    lints.push(
        SyntaxLint::new(
            SyntaxLintCode::ExplicitDeclId,
            format!("`{kind} @{id}` spells the default declaration family explicitly; `{kind} {name}` is the compact authoring form"),
            range,
        )
        .with_suggestion(SyntaxLintSuggestion::machine_applicable(
            format!("replace explicit `@{id}` with compact `{name}`"),
            SyntaxLintEdit::new(range, name),
        )),
    );
}

fn lint_generated_surface_form(
    kind: &str,
    id: &str,
    name: &str,
    range: TextRange,
    attrs: &[Attribute],
    source_attrs: &[Attribute],
    lints: &mut Vec<SyntaxLint>,
) {
    if allows_lint(attrs, source_attrs, SyntaxLintCode::GeneratedSurfaceForm) {
        return;
    }
    lints.push(SyntaxLint::new(
        SyntaxLintCode::GeneratedSurfaceForm,
        format!("`{kind} @{id} {name}` is a generated or fully elaborated declaration surface"),
        range,
    ));
}

fn is_generated(attrs: &[Attribute], source_attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .chain(source_attrs.iter())
        .any(|attr| attr.name() == "generated")
}

fn allows_lint(attrs: &[Attribute], source_attrs: &[Attribute], code: SyntaxLintCode) -> bool {
    attrs
        .iter()
        .chain(source_attrs.iter())
        .any(|attr| attr_allows_lint(attr, code))
}

fn attr_allows_lint(attr: &Attribute, code: SyntaxLintCode) -> bool {
    attr.name() == "allow"
        && attr.args().is_some_and(|args| {
            args.split(',')
                .map(str::trim)
                .any(|arg| arg == code.domain_name() || arg == code.stable_code())
        })
}

fn lint_flow_item_ids(item: &FlowItem, lints: &mut Vec<SyntaxLint>) {
    match item {
        FlowItem::Stmt(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
        FlowItem::SpeakerLine(line) => {
            lint_optional_id(line.options().id(), lints);
            lint_optional_id(line.options().text_key(), lints);
        }
        FlowItem::ContentCall(call) => {
            lint_optional_id(call.options().id(), lints);
            lint_optional_id(call.options().text_key(), lints);
        }
        FlowItem::Choice(choice) => {
            lint_optional_id(choice.id(), lints);
            for item in choice.items() {
                lint_choice_item_ids(item, lints);
            }
        }
        FlowItem::If(block) => lint_flow_items(block.body(), lints),
        FlowItem::IfLet(block) => lint_flow_items(block.body(), lints),
        FlowItem::Match(block) => {
            for arm in block.arms() {
                lint_flow_items(arm.body(), lints);
            }
        }
        FlowItem::Loop(block) => lint_flow_items(block.body(), lints),
        FlowItem::While(block) => lint_flow_items(block.body(), lints),
        FlowItem::WhileLet(block) => lint_flow_items(block.body(), lints),
        FlowItem::For(block) => lint_flow_items(block.body(), lints),
        FlowItem::Select(block) => {
            for branch in block.branches() {
                lint_flow_items(branch.body(), lints);
            }
        }
        FlowItem::BorrowBlock(block) => lint_flow_items(block.body(), lints),
        FlowItem::SourceLocale(block) => lint_flow_items(block.body(), lints),
        FlowItem::Scope(block) => lint_flow_items(block.body(), lints),
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                lint_flow_items(branch.body(), lints);
            }
        }
    }
}

fn lint_flow_items(items: &[FlowItem], lints: &mut Vec<SyntaxLint>) {
    for item in items {
        lint_flow_item_ids(item, lints);
    }
}

fn lint_choice_item_ids(item: &ChoiceItem, lints: &mut Vec<SyntaxLint>) {
    match item {
        ChoiceItem::Option(option) => {
            lint_optional_id(option.id(), lints);
            lint_optional_id(option.label_text_key(), lints);
            if let ChoiceAction::Goto(target) = option.action()
                && let Some(relative) = target
                    .family_relative_ref()
                    .map(FamilyRelativeEntityRef::relative)
            {
                lint_relative_id(relative, lints);
            }
        }
        ChoiceItem::If { items, .. } | ChoiceItem::For { items, .. } => {
            for item in items {
                lint_choice_item_ids(item, lints);
            }
        }
        ChoiceItem::Match { arms, .. } => {
            for arm in arms {
                for item in arm.items() {
                    lint_choice_item_ids(item, lints);
                }
            }
        }
        ChoiceItem::Let { .. } | ChoiceItem::Raw(_) => {}
    }
}

fn lint_optional_id(id: Option<&IdRef>, lints: &mut Vec<SyntaxLint>) {
    if let Some(relative) = id.and_then(IdRef::relative_id) {
        lint_relative_id(relative, lints);
    }
}

fn lint_relative_id(relative: &RelativeId, lints: &mut Vec<SyntaxLint>) {
    if relative.spelling() == RelativeIdSpelling::DotRun && relative.parent_depth() >= 2 {
        let replacement = explicit_super_relative_id(relative);
        lints.push(
            SyntaxLint::new(
                SyntaxLintCode::DeepDotRunRelativeId,
                format!(
                    "`@...{}` is accepted but hand-written source should prefer explicit `{replacement}`",
                    relative.suffix(),
                ),
                *relative.range(),
            )
            .with_suggestion(SyntaxLintSuggestion::machine_applicable(
                format!("replace dot-run relative id with `{replacement}`"),
                SyntaxLintEdit::new(*relative.range(), replacement),
            )),
        );
    }
}

fn explicit_super_relative_id(relative: &RelativeId) -> String {
    let mut replacement = String::from("@");
    for depth in 0..relative.parent_depth() {
        if depth > 0 {
            replacement.push('.');
        }
        replacement.push_str("super");
    }
    replacement.push('.');
    replacement.push_str(relative.suffix());
    replacement
}

impl SyntaxLint {
    fn new(code: SyntaxLintCode, message: String, range: TextRange) -> Self {
        Self {
            code,
            message,
            range,
            suggestions: Vec::new(),
        }
    }

    #[must_use]
    fn with_suggestion(mut self, suggestion: SyntaxLintSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub const fn code(&self) -> SyntaxLintCode {
        self.code
    }

    pub const fn severity(&self) -> SyntaxLintSeverity {
        self.code.default_severity()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub fn suggestions(&self) -> &[SyntaxLintSuggestion] {
        &self.suggestions
    }

    /// Builds a structured diagnostic for CLI, LSP, and Agent tooling.
    pub fn diagnostic(&self, source: &SourceName) -> Diagnostic {
        let span = SourceSpan::new(
            source.clone(),
            SourceRange::new(self.range.start(), self.range.end()),
        );
        self.suggestions.iter().fold(
            Diagnostic::new(self.severity().diagnostic_severity(), self.message.clone())
                .with_code(self.code.stable_code())
                .with_label(DiagnosticLabel::primary(
                    span,
                    Some(self.code.domain_name().to_owned()),
                )),
            |diagnostic, suggestion| diagnostic.with_suggestion(suggestion.diagnostic(source)),
        )
    }
}

impl SyntaxLintEdit {
    fn new(range: TextRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    fn source_edit(&self, source: SourceName) -> SourceEdit {
        SourceEdit::new(
            SourceSpan::new(
                source,
                SourceRange::new(self.range.start(), self.range.end()),
            ),
            self.replacement.clone(),
        )
    }
}

impl SyntaxLintSuggestion {
    fn machine_applicable(message: impl Into<String>, edit: SyntaxLintEdit) -> Self {
        Self {
            message: message.into(),
            edits: vec![edit],
            applicability: DiagnosticApplicability::MachineApplicable,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn edits(&self) -> &[SyntaxLintEdit] {
        &self.edits
    }

    pub const fn applicability(&self) -> DiagnosticApplicability {
        self.applicability
    }

    fn diagnostic(&self, source: &SourceName) -> DiagnosticSuggestion {
        self.edits.iter().fold(
            DiagnosticSuggestion::new(self.message.clone(), self.applicability),
            |suggestion, edit| suggestion.with_edit(edit.source_edit(source.clone())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;

    fn lint_codes(source: &str) -> Vec<SyntaxLintCode> {
        let parsed = parse_source(source);
        lint_id_policy(parsed.typed_tree())
            .into_iter()
            .map(|lint| lint.code())
            .collect()
    }

    #[test]
    fn lints_redundant_flow_and_source_decl_identity() {
        let codes = lint_codes(
            r"
flow @flow.opening opening {
}

source @source.http_requests http_requests: Source<HttpRequest, HttpError> {
}
",
        );

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == SyntaxLintCode::RedundantDeclIdentity)
                .count(),
            2
        );
    }

    #[test]
    fn lints_decl_binding_mismatch_as_identity_error() {
        let codes = lint_codes(
            r"
flow @flow.opening start {
}

source @source.http_requests local_requests: Source<HttpRequest, HttpError> {
}
",
        );

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == SyntaxLintCode::DeclBindingMismatch)
                .count(),
            2
        );
    }

    #[test]
    fn surface_alias_is_decl_identity_name() {
        let codes = lint_codes(
            r"
pub surface character @character.alice Alice as alice {
}
",
        );

        assert!(!codes.contains(&SyntaxLintCode::DeclBindingMismatch));
        assert!(codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn generated_marker_surfaces_generated_decl_form() {
        let codes = lint_codes(
            r"
#[generated]
flow @flow.opening opening {
}
",
        );

        assert!(codes.contains(&SyntaxLintCode::GeneratedSurfaceForm));
        assert!(!codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn inner_generated_marker_applies_to_source_decl_forms() {
        let codes = lint_codes(
            r"
#![generated(tool)]
flow @flow.opening opening {
}

source @source.http_requests http_requests: Source<HttpRequest, HttpError> {
}
",
        );

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == SyntaxLintCode::GeneratedSurfaceForm)
                .count(),
            2
        );
        assert!(!codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn inner_generated_marker_does_not_suppress_decl_mismatch() {
        let codes = lint_codes(
            r"
#![generated(tool)]
flow @flow.opening start {
}
",
        );

        assert!(codes.contains(&SyntaxLintCode::DeclBindingMismatch));
    }

    #[test]
    fn allow_attribute_suppresses_flow_module_mismatch() {
        let codes = lint_codes(
            r"
mod route::opening

#[allow(id::flow_module_mismatch)]
flow @flow.prologue {
}
",
        );

        assert!(!codes.contains(&SyntaxLintCode::FlowIdModuleMismatch));
    }

    #[test]
    fn inner_allow_attribute_suppresses_source_wide_id_lints() {
        let codes = lint_codes(
            r"
#![allow(id::flow_module_mismatch, style::redundant_decl_identity)]
mod route::opening

flow @flow.prologue prologue {
}
",
        );

        assert!(!codes.contains(&SyntaxLintCode::FlowIdModuleMismatch));
        assert!(!codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn misplaced_inner_attribute_does_not_apply_to_source_lints() {
        let parsed = parse_source(
            r"
flow @flow.opening opening {
}

#![generated(tool)]
flow @flow.generated generated {
}
",
        );

        assert!(parsed.errors().iter().any(|error| {
            error
                .message()
                .contains("inner source attribute must appear before")
        }));
        assert!(parsed.typed_tree().attrs().is_empty());
        let codes = lint_id_policy(parsed.typed_tree())
            .into_iter()
            .map(|lint| lint.code())
            .collect::<Vec<_>>();
        assert!(codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
        assert!(!codes.contains(&SyntaxLintCode::GeneratedSurfaceForm));
    }

    #[test]
    fn explicit_decl_id_has_stable_hint_code() {
        let parsed = parse_source(
            r"
flow @flow.opening {
}
",
        );
        let lint = lint_id_policy(parsed.typed_tree())
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .expect("explicit id lint");

        assert_eq!(lint.code().stable_code(), "AWF0103");
        assert_eq!(lint.severity(), SyntaxLintSeverity::Hint);
    }

    #[test]
    fn explicit_entity_decl_id_prefers_compact_authoring_form() {
        let parsed = parse_source(
            r"
asset @asset.bg_room {
}

content @content.chapter_two {
    roots = []
}
",
        );
        let lints = lint_id_policy(parsed.typed_tree());
        let explicit = lints
            .iter()
            .filter(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .collect::<Vec<_>>();

        assert_eq!(explicit.len(), 2);
        assert!(explicit.iter().any(|lint| {
            lint.message().contains("asset bg_room")
                && lint
                    .message()
                    .contains("default declaration family explicitly")
        }));
        assert!(
            explicit
                .iter()
                .any(|lint| lint.message().contains("content chapter_two"))
        );
    }

    #[test]
    fn bare_entity_decl_name_is_canonical_not_redundant() {
        let codes = lint_codes(
            r"
character alice {
}
",
        );

        assert!(!codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
        assert!(!codes.contains(&SyntaxLintCode::DeclBindingMismatch));
    }

    #[test]
    fn explicit_id_lint_carries_machine_applicable_suggestion() {
        let parsed = parse_source("flow @flow.opening {\n}\n");
        let lint = lint_id_policy(parsed.typed_tree())
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .expect("explicit id lint");

        assert_eq!(lint.suggestions().len(), 1);
        assert_eq!(lint.suggestions()[0].edits()[0].replacement(), "opening");
    }

    #[test]
    fn deep_dot_run_lint_carries_explicit_super_suggestion() {
        let parsed = parse_source(
            r#"
flow @flow.opening opening {
    choice {
        @...ending "Next" -> @flow.ending
    }
}
"#,
        );
        let lint = lint_id_policy(parsed.typed_tree())
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::DeepDotRunRelativeId)
            .expect("deep dot-run lint");

        assert_eq!(lint.suggestions().len(), 1);
        assert_eq!(
            lint.suggestions()[0].edits()[0].replacement(),
            "@super.super.ending"
        );
    }
}
