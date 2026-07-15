//! Canonical native Style sheet, patch, and application ownership.

use super::{
    ViewAlignment, ViewPropertyKind, ViewSpecifiedValue, ViewStyleSelector, ViewStyleValueKind,
};
use crate::ViewElementKind;
use crate::{ViewLocalPartName, ViewPartName};
use arcweft_id::{IdError, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

mod codec;

/// Public identity of one named Style sheet.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleSheetId(PublicId);

/// Sheet-local identity of one Style token.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleTokenId(PublicId);

/// Stable identity of one inline Style patch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ViewStylePatchId(u32);

/// Index into the resource-owned source/provenance table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ViewStyleSourceId(u32);

/// Runtime identity of one Style application scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleScopeId(u64);

/// Assignment behavior after the property's value has been checked.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleAssignOp {
    #[default]
    Replace,
    Append,
}

/// Reference carried by an ordered Style application instruction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum ViewStyleApplicationTarget {
    Named { sheet: ViewStyleSheetId },
    Inline { patch: ViewStylePatchId },
}

/// Boundary facts recorded where one Style application enters a View scope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewStyleBoundaryFacts {
    crossed_view_boundaries: u16,
    exported_part: bool,
    inherited_root: bool,
}

/// One ordered sheet or inline-patch application in a retained View scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewStyleApplication {
    target: ViewStyleApplicationTarget,
    scope: ViewStyleScopeId,
    scope_depth: u16,
    application_order: u32,
    boundary: ViewStyleBoundaryFacts,
}

/// One checked token owned by exactly one [`ViewStyleSheet`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleToken {
    id: ViewStyleTokenId,
    value_kind: ViewStyleValueKind,
    value: ViewSpecifiedValue,
    source: ViewStyleSourceId,
}

/// One checked property assignment in a native Style rule or patch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleDeclaration {
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    op: ViewStyleAssignOp,
    source: ViewStyleSourceId,
}

/// One checked selector rule owned by exactly one [`ViewStyleSheet`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleRule {
    selector: ViewStyleSelector,
    declarations: Vec<ViewStyleDeclaration>,
    source_order: u32,
    source: ViewStyleSourceId,
}

/// Canonical native inventory for one named sheet.
///
/// Keeping the ID here means a sheet ID has one canonical owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleSheet {
    id: ViewStyleSheetId,
    tokens: Vec<ViewStyleToken>,
    rules: Vec<ViewStyleRule>,
}

/// Checked native declarations for one inline patch.
///
/// Patch identity and typed declarations are retained without a second
/// product-side projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStylePatch {
    id: ViewStylePatchId,
    declarations: Vec<ViewStyleDeclaration>,
}

/// Canonical sheet-owned native Style program.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ViewStyleProgram {
    sheets: Vec<ViewStyleSheet>,
    patches: Vec<ViewStylePatch>,
}

/// Failure to construct or decode checked canonical Style data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleModelError {
    #[error("duplicate Style sheet {0:?}")]
    DuplicateSheet(ViewStyleSheetId),
    #[error("duplicate inline Style patch {0:?}")]
    DuplicatePatch(ViewStylePatchId),
    #[error(
        "inline Style patch {patch:?} property {property:?} references missing sheet token {referenced:?}"
    )]
    MissingInlineToken {
        patch: ViewStylePatchId,
        property: ViewPropertyKind,
        referenced: ViewStyleTokenId,
    },
    #[error(
        "inline Style patch {patch:?} property {property:?} references ambiguous token {referenced:?} owned by {sheet_count} sheets"
    )]
    AmbiguousInlineToken {
        patch: ViewStylePatchId,
        property: ViewPropertyKind,
        referenced: ViewStyleTokenId,
        sheet_count: usize,
    },
    #[error(
        "inline Style patch {patch:?} property {property:?} expects token {referenced:?} to be {expected:?}, found {actual:?}"
    )]
    InlineTokenKindMismatch {
        patch: ViewStylePatchId,
        property: ViewPropertyKind,
        referenced: ViewStyleTokenId,
        expected: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
    #[error("Style sheets are not in canonical order: {previous:?} precedes {next:?}")]
    NonCanonicalSheetOrder {
        previous: ViewStyleSheetId,
        next: ViewStyleSheetId,
    },
    #[error("inline Style patches are not in canonical order: {previous:?} precedes {next:?}")]
    NonCanonicalPatchOrder {
        previous: ViewStylePatchId,
        next: ViewStylePatchId,
    },
    #[error("token {token:?} declares {declared:?} but contains a value of kind {actual:?}")]
    TokenValueKindMismatch {
        token: ViewStyleTokenId,
        declared: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
    #[error("property {property:?} expects {expected:?} but contains a value of kind {actual:?}")]
    DeclarationValueKindMismatch {
        property: ViewPropertyKind,
        expected: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
    #[error("axis context at source {style_source:?} does not support append assignment")]
    AxisContextAppend { style_source: ViewStyleSourceId },
    #[error("logical translation {property:?} at source {style_source:?} is not sign-reversible")]
    LogicalTranslationNotSignReversible {
        property: ViewPropertyKind,
        style_source: ViewStyleSourceId,
    },
    #[error("property {property:?} does not support append assignment")]
    InvalidAppend { property: ViewPropertyKind },
    #[error("alignment {alignment:?} is not valid for property {property:?}")]
    InvalidAlignment {
        property: ViewPropertyKind,
        alignment: ViewAlignment,
    },
    #[error("Style rule at source order {source_order} has no declarations")]
    EmptyRule { source_order: u32 },
    #[error("duplicate Style token {0:?}")]
    DuplicateToken(ViewStyleTokenId),
    #[error("duplicate Style rule source order {0}")]
    DuplicateRuleSourceOrder(u32),
    #[error("token {owner:?} references missing sheet-local token {referenced:?}")]
    UnknownTokenReference {
        owner: ViewStyleTokenId,
        referenced: ViewStyleTokenId,
    },
    #[error(
        "token {owner:?} expects referenced token {referenced:?} to be {expected:?}, found {actual:?}"
    )]
    TokenReferenceKindMismatch {
        owner: ViewStyleTokenId,
        referenced: ViewStyleTokenId,
        expected: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
    #[error(
        "rule {source_order} property {property:?} references missing sheet-local token {referenced:?}"
    )]
    UnknownRuleTokenReference {
        source_order: u32,
        property: ViewPropertyKind,
        referenced: ViewStyleTokenId,
    },
    #[error(
        "rule {source_order} property {property:?} expects token {referenced:?} to be {expected:?}, found {actual:?}"
    )]
    RuleTokenReferenceKindMismatch {
        source_order: u32,
        property: ViewPropertyKind,
        referenced: ViewStyleTokenId,
        expected: ViewStyleValueKind,
        actual: ViewStyleValueKind,
    },
    #[error("Style token graph contains a cycle through {0:?}")]
    TokenCycle(ViewStyleTokenId),
    #[error("Style token {token:?} has reference depth {depth}, exceeding the maximum {max_depth}")]
    TokenReferenceDepthExceeded {
        token: ViewStyleTokenId,
        depth: usize,
        max_depth: usize,
    },
    #[error("Style tokens are not in canonical order: {previous:?} precedes {next:?}")]
    NonCanonicalTokenOrder {
        previous: ViewStyleTokenId,
        next: ViewStyleTokenId,
    },
    #[error("Style rules are not in canonical source order: {previous} precedes {next}")]
    NonCanonicalRuleOrder { previous: u32, next: u32 },
    #[error("property {property:?} in rule {source_order} does not apply to element {element:?}")]
    PropertyNotApplicable {
        property: ViewPropertyKind,
        element: ViewElementKind,
        source_order: u32,
    },
}

impl ViewStyleSheetId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl ViewStyleTokenId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl ViewStylePatchId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl ViewStyleSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl ViewStyleScopeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewStyleApplicationTarget {
    pub const fn named(sheet: ViewStyleSheetId) -> Self {
        Self::Named { sheet }
    }

    pub const fn inline(patch: ViewStylePatchId) -> Self {
        Self::Inline { patch }
    }
}

impl ViewStyleBoundaryFacts {
    pub const SAME_VIEW: Self = Self {
        crossed_view_boundaries: 0,
        exported_part: false,
        inherited_root: false,
    };

    pub const fn nested_view(
        crossed_view_boundaries: u16,
        exported_part: bool,
        inherited_root: bool,
    ) -> Self {
        Self {
            crossed_view_boundaries,
            exported_part,
            inherited_root,
        }
    }

    pub const fn is_nested_view_boundary(self) -> bool {
        self.crossed_view_boundaries != 0
    }

    /// Number of nested View boundaries crossed by the owning application.
    pub const fn crossed_view_boundaries(self) -> u16 {
        self.crossed_view_boundaries
    }

    pub const fn is_exported_part(self) -> bool {
        self.exported_part
    }

    pub const fn allows_inherited_root(self) -> bool {
        self.crossed_view_boundaries == 1 && self.inherited_root
    }

    pub const fn allows_selector_traversal(self) -> bool {
        self.crossed_view_boundaries == 0
            || (self.crossed_view_boundaries == 1 && self.exported_part)
    }

    /// Matches in exactly one namespace selected by the crossed boundary.
    pub fn matches_part(
        self,
        selector: &ViewPartName,
        implementation_part: Option<&ViewLocalPartName>,
        exported_part: Option<&ViewPartName>,
    ) -> bool {
        if self.crossed_view_boundaries == 0 {
            implementation_part.is_some_and(|part| part.matches_selector(selector))
        } else if self.crossed_view_boundaries == 1 && self.exported_part {
            exported_part == Some(selector)
        } else {
            false
        }
    }
}

impl ViewStyleApplication {
    pub const fn new(
        target: ViewStyleApplicationTarget,
        scope: ViewStyleScopeId,
        scope_depth: u16,
        application_order: u32,
        boundary: ViewStyleBoundaryFacts,
    ) -> Self {
        Self {
            target,
            scope,
            scope_depth,
            application_order,
            boundary,
        }
    }

    pub const fn target(&self) -> &ViewStyleApplicationTarget {
        &self.target
    }

    pub const fn scope(&self) -> ViewStyleScopeId {
        self.scope
    }

    pub const fn scope_depth(&self) -> u16 {
        self.scope_depth
    }

    pub const fn application_order(&self) -> u32 {
        self.application_order
    }

    pub const fn boundary(&self) -> ViewStyleBoundaryFacts {
        self.boundary
    }
}

impl ViewStyleToken {
    pub fn new(
        id: ViewStyleTokenId,
        value_kind: ViewStyleValueKind,
        value: ViewSpecifiedValue,
        source: ViewStyleSourceId,
    ) -> Result<Self, ViewStyleModelError> {
        let actual = value.kind();
        if actual != value_kind {
            return Err(ViewStyleModelError::TokenValueKindMismatch {
                token: id,
                declared: value_kind,
                actual,
            });
        }
        Ok(Self {
            id,
            value_kind,
            value,
            source,
        })
    }

    pub const fn id(&self) -> &ViewStyleTokenId {
        &self.id
    }

    pub const fn value_kind(&self) -> ViewStyleValueKind {
        self.value_kind
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn source(&self) -> ViewStyleSourceId {
        self.source
    }
}

impl ViewStyleDeclaration {
    pub fn new(
        property: ViewPropertyKind,
        value: ViewSpecifiedValue,
        op: ViewStyleAssignOp,
        source: ViewStyleSourceId,
    ) -> Result<Self, ViewStyleModelError> {
        let expected = property.value_kind();
        let actual = value.kind();
        if actual != expected {
            return Err(ViewStyleModelError::DeclarationValueKindMismatch {
                property,
                expected,
                actual,
            });
        }
        if op == ViewStyleAssignOp::Append && property.is_axis_context() {
            return Err(ViewStyleModelError::AxisContextAppend {
                style_source: source,
            });
        }
        if matches!(
            property,
            ViewPropertyKind::TranslateInline | ViewPropertyKind::TranslateBlock
        ) && matches!(
            &value,
            ViewSpecifiedValue::Length { value } if !value.is_axis_sign_reversible()
        ) {
            return Err(ViewStyleModelError::LogicalTranslationNotSignReversible {
                property,
                style_source: source,
            });
        }
        if op == ViewStyleAssignOp::Append && !property.is_appendable() {
            return Err(ViewStyleModelError::InvalidAppend { property });
        }
        if let ViewSpecifiedValue::Alignment { value: alignment } = &value
            && !alignment.applies_to(property)
        {
            return Err(ViewStyleModelError::InvalidAlignment {
                property,
                alignment: *alignment,
            });
        }
        Ok(Self {
            property,
            value,
            op,
            source,
        })
    }

    pub const fn property(&self) -> ViewPropertyKind {
        self.property
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn op(&self) -> ViewStyleAssignOp {
        self.op
    }

    pub const fn source(&self) -> ViewStyleSourceId {
        self.source
    }
}

impl ViewStyleRule {
    pub fn new(
        selector: ViewStyleSelector,
        declarations: Vec<ViewStyleDeclaration>,
        source_order: u32,
        source: ViewStyleSourceId,
    ) -> Result<Self, ViewStyleModelError> {
        if declarations.is_empty() {
            return Err(ViewStyleModelError::EmptyRule { source_order });
        }
        Ok(Self {
            selector,
            declarations,
            source_order,
            source,
        })
    }

    pub const fn selector(&self) -> &ViewStyleSelector {
        &self.selector
    }

    pub fn declarations(&self) -> &[ViewStyleDeclaration] {
        &self.declarations
    }

    pub const fn source_order(&self) -> u32 {
        self.source_order
    }

    pub const fn source(&self) -> ViewStyleSourceId {
        self.source
    }
}

impl ViewStyleSheet {
    /// Hard safety bound for one sheet-local token-reference chain.
    pub const MAX_TOKEN_REFERENCE_DEPTH: usize = 64;

    pub fn new(
        id: ViewStyleSheetId,
        mut tokens: Vec<ViewStyleToken>,
        mut rules: Vec<ViewStyleRule>,
    ) -> Result<Self, ViewStyleModelError> {
        let token_indices = index_tokens(&tokens)?;
        validate_token_graph(&tokens, &token_indices)?;
        validate_rules(&rules, &tokens, &token_indices)?;
        tokens.sort_by(|left, right| left.id().cmp(right.id()));
        rules.sort_by_key(ViewStyleRule::source_order);
        Ok(Self { id, tokens, rules })
    }

    pub(super) fn from_canonical_parts(
        id: ViewStyleSheetId,
        tokens: Vec<ViewStyleToken>,
        rules: Vec<ViewStyleRule>,
    ) -> Result<Self, ViewStyleModelError> {
        if let Some(pair) = tokens.windows(2).find(|pair| pair[0].id() > pair[1].id()) {
            return Err(ViewStyleModelError::NonCanonicalTokenOrder {
                previous: pair[0].id().clone(),
                next: pair[1].id().clone(),
            });
        }
        if let Some(pair) = rules
            .windows(2)
            .find(|pair| pair[0].source_order() > pair[1].source_order())
        {
            return Err(ViewStyleModelError::NonCanonicalRuleOrder {
                previous: pair[0].source_order(),
                next: pair[1].source_order(),
            });
        }
        Self::new(id, tokens, rules)
    }

    pub const fn id(&self) -> &ViewStyleSheetId {
        &self.id
    }

    pub fn tokens(&self) -> &[ViewStyleToken] {
        &self.tokens
    }

    pub fn rules(&self) -> &[ViewStyleRule] {
        &self.rules
    }

    pub fn token(&self, id: &ViewStyleTokenId) -> Option<&ViewStyleToken> {
        self.tokens
            .binary_search_by(|token| token.id().cmp(id))
            .ok()
            .map(|index| &self.tokens[index])
    }
}

impl ViewStylePatch {
    pub const fn new(id: ViewStylePatchId, declarations: Vec<ViewStyleDeclaration>) -> Self {
        Self { id, declarations }
    }

    pub const fn id(&self) -> ViewStylePatchId {
        self.id
    }

    pub fn declarations(&self) -> &[ViewStyleDeclaration] {
        &self.declarations
    }
}

impl ViewStyleProgram {
    pub fn try_new(
        mut sheets: Vec<ViewStyleSheet>,
        mut patches: Vec<ViewStylePatch>,
    ) -> Result<Self, ViewStyleModelError> {
        let mut sheet_ids = BTreeSet::new();
        for sheet in &sheets {
            if !sheet_ids.insert(sheet.id()) {
                return Err(ViewStyleModelError::DuplicateSheet(sheet.id().clone()));
            }
        }
        let mut patch_ids = BTreeSet::new();
        for patch in &patches {
            if !patch_ids.insert(patch.id()) {
                return Err(ViewStyleModelError::DuplicatePatch(patch.id()));
            }
        }
        validate_inline_patch_tokens(&sheets, &patches)?;
        sheets.sort_by(|left, right| left.id().cmp(right.id()));
        patches.sort_by_key(ViewStylePatch::id);
        Ok(Self { sheets, patches })
    }

    pub(super) fn from_canonical_parts(
        sheets: Vec<ViewStyleSheet>,
        patches: Vec<ViewStylePatch>,
    ) -> Result<Self, ViewStyleModelError> {
        if let Some(pair) = sheets.windows(2).find(|pair| pair[0].id() >= pair[1].id()) {
            return Err(ViewStyleModelError::NonCanonicalSheetOrder {
                previous: pair[0].id().clone(),
                next: pair[1].id().clone(),
            });
        }
        if let Some(pair) = patches.windows(2).find(|pair| pair[0].id() >= pair[1].id()) {
            return Err(ViewStyleModelError::NonCanonicalPatchOrder {
                previous: pair[0].id(),
                next: pair[1].id(),
            });
        }
        Self::try_new(sheets, patches)
    }

    pub fn sheets(&self) -> &[ViewStyleSheet] {
        &self.sheets
    }

    pub fn patches(&self) -> &[ViewStylePatch] {
        &self.patches
    }

    pub fn sheet(&self, id: &ViewStyleSheetId) -> Option<&ViewStyleSheet> {
        self.sheets
            .binary_search_by(|sheet| sheet.id().cmp(id))
            .ok()
            .map(|index| &self.sheets[index])
    }

    pub fn patch(&self, id: ViewStylePatchId) -> Option<&ViewStylePatch> {
        self.patches
            .binary_search_by_key(&id, ViewStylePatch::id)
            .ok()
            .map(|index| &self.patches[index])
    }

    pub fn resolve_token(
        &self,
        sheet: &ViewStyleSheetId,
        token: &ViewStyleTokenId,
    ) -> Option<&ViewStyleToken> {
        self.sheet(sheet).and_then(|sheet| sheet.token(token))
    }
}

fn validate_inline_patch_tokens(
    sheets: &[ViewStyleSheet],
    patches: &[ViewStylePatch],
) -> Result<(), ViewStyleModelError> {
    let mut owners = HashMap::<&ViewStyleTokenId, (ViewStyleValueKind, usize)>::new();
    for token in sheets.iter().flat_map(ViewStyleSheet::tokens) {
        owners
            .entry(token.id())
            .and_modify(|owner| owner.1 += 1)
            .or_insert((token.value_kind(), 1));
    }

    for patch in patches {
        for declaration in patch.declarations() {
            let Some((referenced, expected)) = declaration.value().token_reference() else {
                continue;
            };
            let Some((actual, sheet_count)) = owners.get(referenced).copied() else {
                return Err(ViewStyleModelError::MissingInlineToken {
                    patch: patch.id(),
                    property: declaration.property(),
                    referenced: referenced.clone(),
                });
            };
            if sheet_count != 1 {
                return Err(ViewStyleModelError::AmbiguousInlineToken {
                    patch: patch.id(),
                    property: declaration.property(),
                    referenced: referenced.clone(),
                    sheet_count,
                });
            }
            if actual != expected {
                return Err(ViewStyleModelError::InlineTokenKindMismatch {
                    patch: patch.id(),
                    property: declaration.property(),
                    referenced: referenced.clone(),
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(())
}

fn index_tokens(
    tokens: &[ViewStyleToken],
) -> Result<HashMap<&ViewStyleTokenId, usize>, ViewStyleModelError> {
    let mut by_id = HashMap::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        if by_id.insert(token.id(), index).is_some() {
            return Err(ViewStyleModelError::DuplicateToken(token.id().clone()));
        }
    }
    Ok(by_id)
}

#[derive(Clone, Copy, Debug, Default)]
enum TokenVisitState {
    #[default]
    Unvisited,
    Visiting,
    Complete(usize),
}

fn validate_token_graph(
    tokens: &[ViewStyleToken],
    by_id: &HashMap<&ViewStyleTokenId, usize>,
) -> Result<(), ViewStyleModelError> {
    let edges = tokens
        .iter()
        .map(|token| {
            let Some((referenced, expected)) = token.value().token_reference() else {
                return Ok(None);
            };
            let Some(&target_index) = by_id.get(referenced) else {
                return Err(ViewStyleModelError::UnknownTokenReference {
                    owner: token.id().clone(),
                    referenced: referenced.clone(),
                });
            };
            let target = &tokens[target_index];
            if target.value_kind() != expected {
                return Err(ViewStyleModelError::TokenReferenceKindMismatch {
                    owner: token.id().clone(),
                    referenced: referenced.clone(),
                    expected,
                    actual: target.value_kind(),
                });
            }
            Ok(Some(target_index))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut states = vec![TokenVisitState::Unvisited; tokens.len()];
    for start in 0..tokens.len() {
        if matches!(states[start], TokenVisitState::Complete(_)) {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        let base_depth = loop {
            match states[current] {
                TokenVisitState::Complete(depth) => break depth,
                TokenVisitState::Visiting => {
                    return Err(ViewStyleModelError::TokenCycle(
                        tokens[current].id().clone(),
                    ));
                }
                TokenVisitState::Unvisited => {
                    states[current] = TokenVisitState::Visiting;
                    path.push(current);
                    if path.len() > ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH {
                        return Err(ViewStyleModelError::TokenReferenceDepthExceeded {
                            token: tokens[start].id().clone(),
                            depth: path.len(),
                            max_depth: ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
                        });
                    }
                    let Some(next) = edges[current] else {
                        break 0;
                    };
                    current = next;
                }
            }
        };
        let depth = base_depth.saturating_add(path.len());
        if depth > ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH {
            return Err(ViewStyleModelError::TokenReferenceDepthExceeded {
                token: tokens[start].id().clone(),
                depth,
                max_depth: ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
            });
        }
        let mut depth = base_depth;
        for index in path.into_iter().rev() {
            depth += 1;
            states[index] = TokenVisitState::Complete(depth);
        }
    }
    Ok(())
}

fn validate_rules(
    rules: &[ViewStyleRule],
    tokens: &[ViewStyleToken],
    token_indices: &HashMap<&ViewStyleTokenId, usize>,
) -> Result<(), ViewStyleModelError> {
    let mut source_orders = BTreeSet::new();
    for rule in rules {
        if !source_orders.insert(rule.source_order()) {
            return Err(ViewStyleModelError::DuplicateRuleSourceOrder(
                rule.source_order(),
            ));
        }
        if let Some(element) = rule.selector().target_element()
            && let Some(declaration) = rule
                .declarations()
                .iter()
                .find(|declaration| !declaration.property().applies_to(element))
        {
            return Err(ViewStyleModelError::PropertyNotApplicable {
                property: declaration.property(),
                element,
                source_order: rule.source_order(),
            });
        }
        for declaration in rule.declarations() {
            let Some((referenced, expected)) = declaration.value().token_reference() else {
                continue;
            };
            let Some(&target_index) = token_indices.get(referenced) else {
                return Err(ViewStyleModelError::UnknownRuleTokenReference {
                    source_order: rule.source_order(),
                    property: declaration.property(),
                    referenced: referenced.clone(),
                });
            };
            let actual = tokens[target_index].value_kind();
            if actual != expected {
                return Err(ViewStyleModelError::RuleTokenReferenceKindMismatch {
                    source_order: rule.source_order(),
                    property: declaration.property(),
                    referenced: referenced.clone(),
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(())
}

impl Serialize for ViewStyleSheetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewStyleSheetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewStyleTokenId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewStyleTokenId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewStyleScopeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ViewStyleScopeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(Self)
    }
}
