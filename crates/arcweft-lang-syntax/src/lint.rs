use crate::ast::common::TextRange;
use crate::attachment::{
    AttachedAttributeArgument, AttachedAttributeValue, AttachedCharacterSurfaceAlias,
    AttachedDeclarationPublicId, AttachedFlowIdSyntax, AttachedFlowIdentity,
    AttachedInnerAttribute, AttachedItemPrefix, AttachedOuterAttribute, AttachedRequiredFlowBody,
    AttachedRetainedName, AttachedStyleId, SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};
use crate::expressions::{ExpressionComponentRole, ExpressionProjection};
use crate::grammar::kinds::SyntaxKind;
use crate::id_ref::{AuthoredIdRef, AuthoredIdRoot, SyntaxIdRefPart, SyntaxIdRefSyntax};
use crate::incremental::ParsedSource;
use arcweft_source::{
    Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity, DiagnosticSuggestion,
    SourceDocument, SourceEdit, SourceRange,
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
    pub(crate) const ALL: [Self; 6] = [
        Self::DeepDotRunRelativeId,
        Self::FlowIdModuleMismatch,
        Self::RedundantDeclIdentity,
        Self::DeclBindingMismatch,
        Self::ExplicitDeclId,
        Self::GeneratedSurfaceForm,
    ];

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

const _: [(); SyntaxLintCode::ALL.len()] = [(); 6];

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

/// Lints ID policy choices from the accepted attached syntax snapshot.
pub fn lint_id_policy(source: &ParsedSource) -> Result<Vec<SyntaxLint>, SyntaxAccessError> {
    let mut lints = Vec::new();
    let source_attrs = source.inner_attributes()?;
    let module_tail = attached_module_tail(source)?;
    for item in source.items()? {
        lint_item_ids(&item, module_tail.as_deref(), &source_attrs, &mut lints)?;
    }
    Ok(lints)
}

fn lint_item_ids(
    item: &TypedItemNode,
    module_tail: Option<&str>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    match item {
        TypedItemNode::Flow(flow) => {
            lint_flow(flow, module_tail, source_attrs, lints)?;
        }
        TypedItemNode::Character(character) => {
            lint_character(character, source_attrs, lints)?;
        }
        TypedItemNode::View(item) => lint_retained_item("view", item, source_attrs, lints)?,
        TypedItemNode::Action(item) => lint_retained_item("action", item, source_attrs, lints)?,
        TypedItemNode::Activity(item) => {
            lint_retained_item("activity", item, source_attrs, lints)?;
        }
        TypedItemNode::Signal(item) => lint_retained_item("signal", item, source_attrs, lints)?,
        TypedItemNode::Metric(item) => lint_retained_item("metric", item, source_attrs, lints)?,
        TypedItemNode::Layer(item) => lint_retained_item("layer", item, source_attrs, lints)?,
        TypedItemNode::Proof(proof) => lint_proof(proof, source_attrs, lints)?,
        TypedItemNode::Style(style) => lint_style(style, source_attrs, lints)?,
        _ => {}
    }
    Ok(())
}

fn lint_character(
    character: &crate::attachment::AstNode<crate::attachment::node::CharacterDeclarationItemKind>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let declaration = character.semantics()?;
    let alias = match declaration.surface_alias() {
        AttachedCharacterSurfaceAlias::Resolved { value, .. } => Some(value.as_str()),
        AttachedCharacterSurfaceAlias::Absent | AttachedCharacterSurfaceAlias::Missing { .. } => {
            None
        }
    };
    lint_retained_identity(
        "character",
        declaration.header(),
        alias,
        declaration.prefix(),
        source_attrs,
        lints,
    );
    Ok(())
}

fn lint_proof(
    proof: &crate::attachment::AstNode<crate::attachment::node::ProofItemKind>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let declaration = proof.semantics()?;
    if let (AttachedDeclarationPublicId::Explicit { syntax, value }, Some(name)) =
        (declaration.public_id(), declaration.name().value())
    {
        lint_decl_identity(
            "proof",
            value.as_str(),
            name.as_str(),
            text_range(syntax.range()),
            declaration.prefix(),
            source_attrs,
            lints,
        );
    }
    Ok(())
}

fn lint_style(
    style: &crate::attachment::AstNode<crate::attachment::node::StyleItemKind>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let declaration = style.semantics()?;
    if let AttachedStyleId::Authored {
        syntax,
        reference,
        form: crate::attachment::StyleIdForm::Explicit,
        ..
    } = declaration.id()
        && let Some(id) = valid_id(reference)
    {
        let id = authored_id_text(id);
        let name = id.rsplit('.').next().unwrap_or("style");
        lint_explicit_decl_id(
            "style",
            &id,
            name,
            text_range(syntax.range()),
            declaration.prefix(),
            source_attrs,
            lints,
        );
    }
    Ok(())
}

fn lint_flow(
    flow: &crate::attachment::AstNode<crate::attachment::node::FlowItemKind>,
    module_tail: Option<&str>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let declaration = flow.semantics()?;
    let prefix = declaration.prefix();
    let (public_id, name) = match declaration.identity() {
        AttachedFlowIdentity::Name { .. } | AttachedFlowIdentity::Missing { .. } => (None, None),
        AttachedFlowIdentity::PublicId { public_id } => (Some(public_id), None),
        AttachedFlowIdentity::PublicIdAndName { public_id, name } => {
            (Some(public_id), Some(name.value().as_str()))
        }
    };
    if let Some(public_id) = public_id
        && let AttachedFlowIdSyntax::Authored(reference) = public_id.value()
        && let Some(reference) = valid_id(reference)
    {
        let id = authored_id_text(reference);
        let id_range = text_range(public_id.syntax().range());
        if let Some(name) = name {
            lint_decl_identity("flow", &id, name, id_range, prefix, source_attrs, lints);
        } else if let Some(name) = id.rsplit('.').next() {
            lint_explicit_decl_id("flow", &id, name, id_range, prefix, source_attrs, lints);
        }
        if module_tail
            != reference
                .segments()
                .last()
                .map(crate::id_ref::AuthoredIdSegment::as_str)
            && !allows_lint(prefix, source_attrs, SyntaxLintCode::FlowIdModuleMismatch)
        {
            lints.push(SyntaxLint::new(
                SyntaxLintCode::FlowIdModuleMismatch,
                format!(
                    "flow id `{id}` does not follow module tail `{}`",
                    module_tail.unwrap_or_default()
                ),
                id_range,
            ));
        }
    }
    if let AttachedRequiredFlowBody::Present(body) = declaration.body() {
        lint_choice_id_subtrees(&body.syntax().syntax(), lints)?;
    }
    Ok(())
}

fn lint_retained_item<K: crate::attachment::ExactAstKind>(
    kind: &str,
    item: &crate::attachment::AstNode<K>,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let Some(header) = TypedItemNode::from_syntax(item.syntax())?.declaration_header()? else {
        return Ok(());
    };
    let header = header.retained_semantics()?;
    let prefix = TypedItemNode::from_syntax(item.syntax())?.attached_prefix()?;
    lint_retained_identity(kind, &header, None, &prefix, source_attrs, lints);
    Ok(())
}

fn lint_retained_identity(
    kind: &str,
    header: &crate::attachment::AttachedRetainedHeader,
    preferred_name: Option<&str>,
    prefix: &AttachedItemPrefix,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) {
    let AttachedDeclarationPublicId::Explicit { syntax, value } = header.public_id() else {
        return;
    };
    let name = preferred_name.or_else(|| match header.name() {
        AttachedRetainedName::Resolved { value, .. } => Some(value.as_str()),
        AttachedRetainedName::Missing { .. } | AttachedRetainedName::Invalid { .. } => None,
    });
    let Some(name) = name else {
        return;
    };
    lint_decl_identity(
        kind,
        value.as_str(),
        name,
        text_range(syntax.range()),
        prefix,
        source_attrs,
        lints,
    );
}

fn lint_decl_identity(
    kind: &str,
    id: &str,
    name: &str,
    range: TextRange,
    prefix: &AttachedItemPrefix,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) {
    let Some(id_tail) = id.rsplit('.').next() else {
        return;
    };
    if id_tail == name {
        if is_generated(prefix, source_attrs) {
            lint_generated_surface_form(kind, id, name, range, prefix, source_attrs, lints);
            return;
        }
        if allows_lint(prefix, source_attrs, SyntaxLintCode::RedundantDeclIdentity) {
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
    prefix: &AttachedItemPrefix,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) {
    if is_generated(prefix, source_attrs) {
        lint_generated_surface_form(kind, id, name, range, prefix, source_attrs, lints);
        return;
    }
    if allows_lint(prefix, source_attrs, SyntaxLintCode::ExplicitDeclId) {
        return;
    }
    let compact = compact_decl_name(kind, id, name);
    lints.push(
        SyntaxLint::new(
            SyntaxLintCode::ExplicitDeclId,
            format!("`{kind} @{id}` spells the default declaration family explicitly; `{kind} {compact}` is the compact authoring form"),
            range,
        )
        .with_suggestion(SyntaxLintSuggestion::machine_applicable(
            format!("replace explicit `@{id}` with compact `{compact}`"),
            SyntaxLintEdit::new(range, compact),
        )),
    );
}

fn compact_decl_name(kind: &str, id: &str, fallback: &str) -> String {
    let prefix = format!("{kind}.");
    id.strip_prefix(&prefix)
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

fn lint_generated_surface_form(
    kind: &str,
    id: &str,
    name: &str,
    range: TextRange,
    prefix: &AttachedItemPrefix,
    source_attrs: &[AttachedInnerAttribute],
    lints: &mut Vec<SyntaxLint>,
) {
    if allows_lint(prefix, source_attrs, SyntaxLintCode::GeneratedSurfaceForm) {
        return;
    }
    lints.push(SyntaxLint::new(
        SyntaxLintCode::GeneratedSurfaceForm,
        format!("`{kind} @{id} {name}` is a generated or fully elaborated declaration surface"),
        range,
    ));
}

fn is_generated(prefix: &AttachedItemPrefix, source_attrs: &[AttachedInnerAttribute]) -> bool {
    prefix
        .attributes()
        .iter()
        .any(|attribute| outer_attribute_name_is(attribute, "generated"))
        || source_attrs
            .iter()
            .any(|attribute| inner_attribute_name_is(attribute, "generated"))
}

fn allows_lint(
    prefix: &AttachedItemPrefix,
    source_attrs: &[AttachedInnerAttribute],
    code: SyntaxLintCode,
) -> bool {
    prefix
        .attributes()
        .iter()
        .any(|attribute| outer_attribute_allows_lint(attribute, code))
        || source_attrs
            .iter()
            .any(|attribute| inner_attribute_allows_lint(attribute, code))
}

fn outer_attribute_name_is(attribute: &AttachedOuterAttribute, expected: &str) -> bool {
    attribute_name_is(attribute.path(), expected)
}

fn inner_attribute_name_is(attribute: &AttachedInnerAttribute, expected: &str) -> bool {
    attribute_name_is(attribute.path(), expected)
}

fn attribute_name_is(path: &crate::attachment::source_file::AttachedPath, expected: &str) -> bool {
    matches!(
        path.root(),
        crate::attachment::source_file::AttachedPathRoot::ImplicitCrate
    ) && path.missing_name().is_none()
        && path.segments().len() == 1
        && path.segments()[0].source_text() == expected
}

fn outer_attribute_allows_lint(attribute: &AttachedOuterAttribute, code: SyntaxLintCode) -> bool {
    attribute_name_is(attribute.path(), "allow")
        && attribute_arguments_allow(attribute.arguments(), code)
}

fn inner_attribute_allows_lint(attribute: &AttachedInnerAttribute, code: SyntaxLintCode) -> bool {
    attribute_name_is(attribute.path(), "allow")
        && attribute_arguments_allow(attribute.arguments(), code)
}

fn attribute_arguments_allow(
    arguments: &[AttachedAttributeArgument],
    code: SyntaxLintCode,
) -> bool {
    arguments.iter().any(|argument| {
        attribute_argument_is(argument, code.domain_name())
            || attribute_argument_is(argument, code.stable_code())
    })
}

fn attribute_argument_is(argument: &AttachedAttributeArgument, expected: &str) -> bool {
    let AttachedAttributeValue::Authored(expression) = argument.value() else {
        return false;
    };
    let Some(path) = expression.path() else {
        return false;
    };
    if !matches!(
        path.root(),
        crate::attachment::source_file::AttachedPathRoot::ImplicitCrate
    ) || path.missing_name().is_some()
    {
        return false;
    }
    path.segments()
        .iter()
        .map(crate::attachment::source_file::AttachedPathSegment::source_text)
        .eq(expected.split("::"))
}

fn attached_module_tail(source: &ParsedSource) -> Result<Option<String>, SyntaxAccessError> {
    for item in source.items()? {
        if let TypedItemNode::Module(module) = item {
            return Ok(module
                .path()?
                .segments()
                .last()
                .map(|segment| segment.source_text().to_owned()));
        }
    }
    Ok(None)
}

fn valid_id(reference: &SyntaxIdRefSyntax) -> Option<&AuthoredIdRef> {
    reference.value().ok()
}

fn authored_id_text(reference: &AuthoredIdRef) -> String {
    let suffix = reference
        .segments()
        .iter()
        .map(crate::id_ref::AuthoredIdSegment::as_str)
        .collect::<Vec<_>>()
        .join(".");
    match reference.root() {
        AuthoredIdRoot::FamilyRelative { family, .. } if !suffix.is_empty() => {
            format!("{}.{}", family.as_str(), suffix)
        }
        AuthoredIdRoot::Absolute { .. }
        | AuthoredIdRoot::Relative { .. }
        | AuthoredIdRoot::FamilyRelative { .. } => suffix,
    }
}

fn lint_choice_id_subtrees(
    root: &SyntaxNodeHandle,
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let mut pending = root.children();
    while let Some(node) = pending.pop() {
        if node.kind() == SyntaxKind::ChoiceExpression {
            lint_choice_entity_references(&node, lints)?;
        } else {
            pending.extend(node.children());
        }
    }
    Ok(())
}

fn lint_choice_entity_references(
    choice: &SyntaxNodeHandle,
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let mut pending = choice.children();
    while let Some(node) = pending.pop() {
        if node.kind() == SyntaxKind::EntityReferenceExpression {
            lint_entity_reference(&node, lints)?;
        }
        pending.extend(node.children());
    }
    Ok(())
}

fn lint_entity_reference(
    syntax: &SyntaxNodeHandle,
    lints: &mut Vec<SyntaxLint>,
) -> Result<(), SyntaxAccessError> {
    let expression = crate::attachment::AttachedExpressionNode::from_syntax(syntax.clone())?;
    let ExpressionProjection::EntityReference(reference) = expression.projection() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    let Some(reference_value) = valid_id(reference) else {
        return Ok(());
    };
    let AuthoredIdRoot::Relative { parent_depth } = reference_value.root() else {
        return Ok(());
    };
    if *parent_depth < 2 {
        return Ok(());
    }
    let Some(first_parent) = expression.component(ExpressionComponentRole::EntityReference(
        SyntaxIdRefPart::ParentMarker { ordinal: 0 },
    )) else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    };
    if syntax.source_text_for_range(first_parent.range()) != "." {
        return Ok(());
    }
    let suffix = reference_value
        .segments()
        .iter()
        .map(crate::id_ref::AuthoredIdSegment::as_str)
        .collect::<Vec<_>>()
        .join(".");
    let replacement = explicit_super_relative_id(*parent_depth, &suffix);
    let range = text_range(expression.whole_source_span().range());
    lints.push(
        SyntaxLint::new(
            SyntaxLintCode::DeepDotRunRelativeId,
            format!(
                "`@...{suffix}` is accepted but hand-written source should prefer explicit `{replacement}`"
            ),
            range,
        )
        .with_suggestion(SyntaxLintSuggestion::machine_applicable(
            format!("replace dot-run relative id with `{replacement}`"),
            SyntaxLintEdit::new(range, replacement),
        )),
    );
    Ok(())
}

fn explicit_super_relative_id(parent_depth: usize, suffix: &str) -> String {
    format!("@{}.{}", vec!["super"; parent_depth].join("."), suffix)
}

const fn text_range(range: SourceRange) -> TextRange {
    TextRange::new(range.start(), range.end())
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
    ///
    /// # Panics
    ///
    /// Panics if `document` is not the exact source document from which this
    /// lint diagnostic was produced.
    pub fn diagnostic(&self, document: &SourceDocument) -> Diagnostic {
        let span = document
            .span(SourceRange::new(self.range.start(), self.range.end()))
            .expect("a syntax lint range belongs to the document that was linted");
        self.suggestions.iter().fold(
            Diagnostic::new(self.severity().diagnostic_severity(), self.message.clone())
                .with_code(self.code.stable_code())
                .with_label(DiagnosticLabel::primary(
                    span,
                    Some(self.code.domain_name().to_owned()),
                )),
            |diagnostic, suggestion| diagnostic.with_suggestion(suggestion.diagnostic(document)),
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

    fn source_edit(&self, document: &SourceDocument) -> SourceEdit {
        SourceEdit::new(
            document
                .span(SourceRange::new(self.range.start(), self.range.end()))
                .expect("a syntax lint edit belongs to the document that was linted"),
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

    fn diagnostic(&self, document: &SourceDocument) -> DiagnosticSuggestion {
        self.edits.iter().fold(
            DiagnosticSuggestion::new(self.message.clone(), self.applicability),
            |suggestion, edit| suggestion.with_edit(edit.source_edit(document)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn parse_lint_fixture(source: impl Into<String>) -> crate::incremental::ParsedSource {
        let name = arcweft_source::SourceName::path("lint.arcw");
        let document = lint_document(&name, source);
        crate::incremental::SyntaxDatabase::try_new()
            .expect("test syntax database")
            .parse_initial(
                arcweft_source::identity::SourceSnapshotId::initial(name),
                document,
                crate::parser::ParseOptions::default(),
            )
            .expect("attached syntax fixture")
    }

    fn lint_document(
        name: &arcweft_source::SourceName,
        source: impl Into<String>,
    ) -> std::sync::Arc<arcweft_source::SourceDocument> {
        std::sync::Arc::new(
            arcweft_source::SourceDocument::try_new(
                arcweft_source::SourceDocumentId::try_new("arcweft-test://syntax/lint")
                    .expect("fixed test document ID is valid"),
                name.clone(),
                source.into(),
            )
            .expect("test source document"),
        )
    }

    fn lint_codes(source: &str) -> Vec<SyntaxLintCode> {
        let parsed = parse_lint_fixture(source);
        lint_id_policy(&parsed)
            .expect("lint projection")
            .into_iter()
            .map(|lint| lint.code())
            .collect()
    }

    #[test]
    fn lint_code_inventory_is_complete_unique_and_stable() {
        let expected = [
            (
                SyntaxLintCode::DeepDotRunRelativeId,
                "AWF0001",
                "id::deep_dot_run",
                SyntaxLintSeverity::Warning,
            ),
            (
                SyntaxLintCode::FlowIdModuleMismatch,
                "AWF0002",
                "id::flow_module_mismatch",
                SyntaxLintSeverity::Warning,
            ),
            (
                SyntaxLintCode::RedundantDeclIdentity,
                "AWF0101",
                "style::redundant_decl_identity",
                SyntaxLintSeverity::Warning,
            ),
            (
                SyntaxLintCode::DeclBindingMismatch,
                "AWF0102",
                "identity::decl_binding_mismatch",
                SyntaxLintSeverity::Error,
            ),
            (
                SyntaxLintCode::ExplicitDeclId,
                "AWF0103",
                "style::explicit_decl_id",
                SyntaxLintSeverity::Hint,
            ),
            (
                SyntaxLintCode::GeneratedSurfaceForm,
                "AWF0104",
                "style::generated_surface_form",
                SyntaxLintSeverity::Information,
            ),
        ];

        assert_eq!(SyntaxLintCode::ALL, expected.map(|entry| entry.0));
        assert_eq!(SyntaxLintCode::ALL.len(), 6);
        assert_eq!(
            SyntaxLintCode::ALL
                .iter()
                .map(|code| code.stable_code())
                .collect::<BTreeSet<_>>()
                .len(),
            SyntaxLintCode::ALL.len()
        );
        assert_eq!(
            SyntaxLintCode::ALL
                .iter()
                .map(|code| code.domain_name())
                .collect::<BTreeSet<_>>()
                .len(),
            SyntaxLintCode::ALL.len()
        );
        for (code, stable_code, domain_name, severity) in expected {
            assert_eq!(code.stable_code(), stable_code);
            assert_eq!(code.domain_name(), domain_name);
            assert_eq!(code.default_severity(), severity);
        }
    }

    #[test]
    fn inner_attribute_projection_is_typed_source_bound_and_recovers_without_a_reader() {
        let parsed =
            parse_lint_fixture("#![allow(id::flow_module_mismatch, AWF0101)]\nflow opening {}\n");
        let attributes = parsed.inner_attributes().unwrap();
        let [attribute] = attributes.as_slice() else {
            panic!("one source attribute")
        };
        assert_eq!(attribute.path().segments()[0].source_text(), "allow");
        assert_eq!(attribute.arguments().len(), 2);
        assert!(attribute_arguments_allow(
            attribute.arguments(),
            SyntaxLintCode::FlowIdModuleMismatch
        ));
        assert!(attribute_arguments_allow(
            attribute.arguments(),
            SyntaxLintCode::RedundantDeclIdentity
        ));
        assert_eq!(
            attribute.syntax().source_span().source(),
            parsed.document().identity()
        );

        let recovered = parse_lint_fixture("#![]\nflow opening {}\n");
        let attributes = recovered.inner_attributes().unwrap();
        assert!(matches!(
            attributes[0].issue(),
            Some(crate::attachment::AttachedOuterAttributeIssue::MissingPath)
        ));
        assert!(lint_id_policy(&recovered).is_ok());
    }

    #[test]
    fn inner_attribute_handles_reject_foreign_and_stale_database_use() {
        use arcweft_source::identity::SourceSnapshotId;
        use arcweft_source::{SourceEdit, SourceRange};

        let name = arcweft_source::SourceName::path("lint-owner.arcw");
        let source = "#![generated(tool)]\nflow opening {}\n";
        let mut database = crate::incremental::SyntaxDatabase::try_new().unwrap();
        let initial = database
            .parse_initial(
                SourceSnapshotId::initial(name.clone()),
                lint_document(&name, source),
                crate::parser::ParseOptions::default(),
            )
            .unwrap();
        let old_attribute = initial.inner_attributes().unwrap()[0].syntax().clone();

        let mut foreign_database = crate::incremental::SyntaxDatabase::try_new().unwrap();
        let foreign = foreign_database
            .parse_initial(
                SourceSnapshotId::initial(name.clone()),
                lint_document(&name, source),
                crate::parser::ParseOptions::default(),
            )
            .unwrap();
        let foreign_attribute = foreign.inner_attributes().unwrap()[0].syntax().clone();
        assert!(matches!(
            database.resolve_current(&foreign_attribute),
            Err(crate::attachment::SyntaxLookupError::WrongDatabase { .. })
        ));

        let start = source.find("opening").unwrap();
        let edit = SourceEdit::new(
            initial
                .document()
                .span(SourceRange::new(start, start + "opening".len()))
                .unwrap(),
            "ending",
        );
        let current = database
            .reparse(&initial, &[edit], crate::parser::ParseOptions::default())
            .unwrap();
        assert!(matches!(
            database.resolve_current(&old_attribute),
            Err(crate::attachment::SyntaxLookupError::StaleGeneration {
                current: current_generation,
                supplied,
            }) if current_generation == current.source_snapshot_id().generation()
                && supplied == initial.source_snapshot_id().generation()
        ));
    }

    #[test]
    fn lints_redundant_flow_and_proof_decl_identity() {
        let codes = lint_codes(
            r"
flow @flow.opening opening {
}

proof @proof.http_requests http_requests {
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
    fn lints_relative_entity_decl_identity_without_source_rescanning() {
        let codes = lint_codes(
            r"
character @.pulse pulse {
}
",
        );

        assert!(codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
        assert!(!codes.contains(&SyntaxLintCode::DeclBindingMismatch));
    }

    #[test]
    fn lints_redundant_proof_decl_identity() {
        let codes = lint_codes(
            r"
proof @proof.opening opening {
}

proof @proof:.relative relative {
}

proof @.short short {
}
",
        );

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == SyntaxLintCode::RedundantDeclIdentity)
                .count(),
            3
        );
        assert!(!codes.contains(&SyntaxLintCode::DeclBindingMismatch));

        let bare_codes = lint_codes(
            r"
proof canonical {
}
",
        );
        assert!(!bare_codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn proof_identity_style_lints_respect_allow_and_generated_attributes() {
        let allowed = lint_codes(
            r"
#[allow(style::redundant_decl_identity)]
proof @proof.allowed allowed {
}
",
        );
        assert!(!allowed.contains(&SyntaxLintCode::RedundantDeclIdentity));

        let generated = lint_codes(
            r"
#[generated]
proof @proof.generated generated {
}
",
        );
        assert!(generated.contains(&SyntaxLintCode::GeneratedSurfaceForm));
        assert!(!generated.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn lints_decl_binding_mismatch_as_identity_error() {
        let codes = lint_codes(
            r"
flow @flow.opening start {
}

proof @proof.http_requests local_requests {
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
pub character @character.alice Alice as alice {
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
    fn generated_marker_surfaces_generated_id_only_form() {
        let codes = lint_codes(
            r"
#[generated]
flow @flow.opening {
}
",
        );

        assert!(codes.contains(&SyntaxLintCode::GeneratedSurfaceForm));
        assert!(!codes.contains(&SyntaxLintCode::ExplicitDeclId));
    }

    #[test]
    fn inner_generated_marker_applies_to_proof_decl_forms() {
        let codes = lint_codes(
            r"
#![generated(tool)]
flow @flow.opening opening {
}

proof @proof.http_requests http_requests {
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
mod route.opening

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
mod route.opening

flow @flow.prologue prologue {
}
",
        );

        assert!(!codes.contains(&SyntaxLintCode::FlowIdModuleMismatch));
        assert!(!codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
    }

    #[test]
    fn misplaced_inner_attribute_does_not_apply_to_source_lints() {
        let parsed = parse_lint_fixture(
            r"
flow @flow.opening opening {
}

#![generated(tool)]
flow @flow.generated generated {
}
",
        );

        assert!(parsed.diagnostics().iter().any(|error| {
            error
                .message()
                .contains("inner source attribute must appear before")
        }));
        assert!(parsed.inner_attributes().unwrap().is_empty());
        let codes = lint_id_policy(&parsed)
            .expect("lint projection")
            .into_iter()
            .map(|lint| lint.code())
            .collect::<Vec<_>>();
        assert!(codes.contains(&SyntaxLintCode::RedundantDeclIdentity));
        assert!(!codes.contains(&SyntaxLintCode::GeneratedSurfaceForm));
    }

    #[test]
    fn explicit_decl_id_has_stable_hint_code() {
        let source = r"
flow @flow.opening {
}
";
        let parsed = parse_lint_fixture(source);
        let lint = lint_id_policy(&parsed)
            .expect("lint projection")
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .expect("explicit id lint");

        assert_eq!(lint.code().stable_code(), "AWF0103");
        assert_eq!(lint.code().domain_name(), "style::explicit_decl_id");
        assert_eq!(lint.severity(), SyntaxLintSeverity::Hint);
        let diagnostic = lint.diagnostic(parsed.document());
        assert_eq!(diagnostic.severity(), DiagnosticSeverity::Hint);
        assert_eq!(
            diagnostic
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("AWF0103")
        );
        assert_eq!(
            diagnostic.labels()[0].message(),
            Some("style::explicit_decl_id")
        );
        assert_eq!(diagnostic.suggestions().len(), 1);
        assert_eq!(
            diagnostic.suggestions()[0].edits()[0].replacement(),
            "opening"
        );
    }

    #[test]
    fn explicit_style_ids_prefer_compact_authoring_form() {
        let parsed = parse_lint_fixture(
            r"
style @.sample.pulse_sprite {
}

style @.chapter_two {
}
",
        );
        let lints = lint_id_policy(&parsed).expect("lint projection");
        let explicit = lints
            .iter()
            .filter(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .collect::<Vec<_>>();

        assert_eq!(explicit.len(), 2);
        assert!(
            explicit
                .iter()
                .any(|lint| lint.message().contains("style sample.pulse_sprite"))
        );
        assert!(
            explicit
                .iter()
                .any(|lint| lint.message().contains("style chapter_two"))
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
        assert!(!codes.contains(&SyntaxLintCode::ExplicitDeclId));
    }

    #[test]
    fn compact_declaration_spelling_does_not_emit_explicit_id_hint() {
        let codes = lint_codes(
            r#"
pub character concierge {
}

pub view ModernFeedbackPanel() {
    Text("ok")
}

flow opening {
}
"#,
        );

        assert!(!codes.contains(&SyntaxLintCode::ExplicitDeclId));
    }

    #[test]
    fn explicit_id_lint_carries_machine_applicable_suggestion() {
        let parsed = parse_lint_fixture("flow @flow.opening {\n}\n");
        let lint = lint_id_policy(&parsed)
            .expect("lint projection")
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
            .expect("explicit id lint");

        assert_eq!(lint.suggestions().len(), 1);
        assert_eq!(lint.suggestions()[0].edits()[0].replacement(), "opening");
    }

    #[test]
    fn deep_dot_run_lint_carries_explicit_super_suggestion() {
        let parsed = parse_lint_fixture(
            r#"
flow @flow.opening opening {
    choice {
        @...ending "Next" -> @flow.ending
    }
}
"#,
        );
        let lint = lint_id_policy(&parsed)
            .expect("lint projection")
            .into_iter()
            .find(|lint| lint.code() == SyntaxLintCode::DeepDotRunRelativeId)
            .expect("deep dot-run lint");

        assert_eq!(lint.suggestions().len(), 1);
        assert_eq!(
            lint.suggestions()[0].edits()[0].replacement(),
            "@super.super.ending"
        );
    }

    #[test]
    fn explicit_super_relative_id_is_not_reclassified_as_a_dot_run() {
        let codes = lint_codes(
            r#"
flow @flow.opening opening {
    choice {
        @super.super.ending "Next" -> @flow.ending
    }
}
"#,
        );

        assert!(!codes.contains(&SyntaxLintCode::DeepDotRunRelativeId));
    }
}
