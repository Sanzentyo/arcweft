//! Private typed carriers for dialogue-application HIR ownership.
//!
//! These carriers intentionally remain disconnected from the public HIR model
//! until the accepted expression arena can replace every public consumer in
//! one compiling series. In particular, this cut does not invent the final
//! `HirDialogueContent` projection ahead of that arena.

#![allow(dead_code)]

use crate::identity::{ExprId, HirModuleId, PatternId, ScopeId, StmtId};
use arcweft_lang_syntax::{
    ast::line_plan::TimelineAssertPolicy, cst::is_identifier, expr::MAX_CALL_ARGUMENTS,
};
use arcweft_source::{SourceDocument, SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

/// Absolute, relative, or family-relative ID retained without source reparsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HirIdRef {
    Absolute(HirEntityReference),
    Relative(HirRelativeId),
    FamilyRelative(HirFamilyRelativeId),
}

/// Normalized absolute entity-reference body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirEntityReference(Box<str>);

/// Normalized suffix shared by relative ID forms.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirIdSuffix(Box<str>);

/// Normalized family name in a family-relative ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirIdFamily(Box<str>);

/// Relative ID lowered independently of its authored marker spelling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirRelativeId {
    suffix: HirIdSuffix,
    parent_depth: usize,
}

/// Family-qualified relative ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirFamilyRelativeId {
    family: HirIdFamily,
    relative: HirRelativeId,
}

/// Invalid normalized ID data supplied by lowering.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirIdRefInvariantError {
    #[error("an absolute HIR entity reference cannot be empty")]
    EmptyAbsolute,
    #[error("a relative HIR ID suffix cannot be empty")]
    EmptySuffix,
    #[error("a normalized relative HIR ID cannot retain an authored `@` marker")]
    AuthoredRelativeMarker,
    #[error("a relative HIR ID suffix must contain non-empty dot-separated segments")]
    InvalidSuffix,
    #[error("a family-relative HIR ID requires one identifier family")]
    InvalidFamily,
}

/// Zero-based argument position bounded by the ordinary-call contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirCallArgumentOrdinal(u16);

/// Invalid ordinary-call argument position.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("call argument ordinal {ordinal} exceeds the {limit}-argument limit")]
pub(crate) struct HirCallArgumentOrdinalError {
    ordinal: usize,
    limit: usize,
}

/// Configuration coordinate owned by one dialogue application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueCoordinate {
    kind: HirDialogueCoordinateKind,
    argument: HirCallArgumentOrdinal,
    value: ExprId,
}

/// Reserved dialogue configuration coordinate family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueCoordinateKind {
    Id,
    TextKey,
}

/// Typed line-plan payload whose children live in the accepted HIR arenas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirLinePlan {
    root_scope: ScopeId,
    label: Option<HirName>,
    items: Box<[HirLinePlanItem]>,
}

/// Semantic name retained without source range ownership.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirName(Box<str>);

/// Direct ID projection of the current line-plan item family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HirLinePlanItem {
    Init(Box<[StmtId]>),
    Thread(StmtId),
    On(StmtId),
    Option {
        name: HirName,
        value: ExprId,
    },
    Let {
        pattern: PatternId,
        value: ExprId,
    },
    Statement(StmtId),
    Out(ExprId),
    CancelRule(StmtId),
    TimedCue {
        anchor: ExprId,
        body: ExprId,
    },
    StartGroup(Box<[HirLinePlanItem]>),
    TogetherGroup(Box<[HirLinePlanItem]>),
    TimelineAssert {
        policy: TimelineAssertPolicy,
        condition: ExprId,
    },
    Expression(ExprId),
    Error(StmtId),
}

/// A line-plan child escaped the module that owns its root scope.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("line-plan child belongs to HIR module {actual:?}, expected {expected:?}")]
pub(crate) struct HirLinePlanInvariantError {
    expected: HirModuleId,
    actual: HirModuleId,
}

/// Invalid semantic name supplied by lowering.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("HIR name `{value}` is not one canonical identifier")]
pub(crate) struct HirNameInvariantError {
    value: Box<str>,
}

/// Bounded unresolved postfix candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HirPostfixBracketCandidates {
    Ambiguous {
        index: ExprId,
        dialogue: ExprId,
    },
    Invalid {
        index: HirPostfixCandidateFailure,
        dialogue: HirPostfixCandidateFailure,
    },
}

/// One typed failure fact for an unresolved postfix interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirPostfixCandidateFailure {
    kind: HirPostfixCandidateFailureKind,
}

/// Grammar reason a postfix interpretation could not be lowered.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

/// Component role in the expression source map.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirExprSourceRole {
    Whole,
    Target,
    OpenBracket,
    CloseBracket,
    Colon,
    Content,
    ContentBody,
    Plan,
    ConfigurationArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
}

/// Whole/name/value component of one ordinary call argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirCallArgumentSourcePart {
    Whole,
    Name,
    Value,
}

/// Revision-bound source span or checked zero-width insertion point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HirSourceSite {
    Span(SourceSpan),
    Insertion(HirInsertionPoint),
}

/// Missing-source component bound to one exact source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirInsertionPoint {
    source: SourceDocumentIdentity,
    offset: usize,
}

/// Invalid insertion point supplied by HIR lowering.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirInsertionPointError {
    #[error("insertion offset {offset} is outside the {document_len}-byte source document")]
    OutOfDocument { offset: usize, document_len: usize },
    #[error("insertion offset {offset} is not a UTF-8 boundary")]
    NonUtf8Boundary { offset: usize },
}

impl HirEntityReference {
    pub(crate) fn try_new(value: impl Into<Box<str>>) -> Result<Self, HirIdRefInvariantError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HirIdRefInvariantError::EmptyAbsolute);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl HirIdSuffix {
    pub(crate) fn try_new(value: impl Into<Box<str>>) -> Result<Self, HirIdRefInvariantError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HirIdRefInvariantError::EmptySuffix);
        }
        if value.contains('@') {
            return Err(HirIdRefInvariantError::AuthoredRelativeMarker);
        }
        if value.split('.').any(str::is_empty) {
            return Err(HirIdRefInvariantError::InvalidSuffix);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl HirIdFamily {
    pub(crate) fn try_new(value: impl Into<Box<str>>) -> Result<Self, HirIdRefInvariantError> {
        let value = value.into();
        if !is_identifier(&value) {
            return Err(HirIdRefInvariantError::InvalidFamily);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl HirRelativeId {
    pub(crate) const fn new(suffix: HirIdSuffix, parent_depth: usize) -> Self {
        Self {
            suffix,
            parent_depth,
        }
    }

    pub(crate) const fn suffix(&self) -> &HirIdSuffix {
        &self.suffix
    }

    pub(crate) const fn parent_depth(&self) -> usize {
        self.parent_depth
    }
}

impl HirFamilyRelativeId {
    pub(crate) const fn new(family: HirIdFamily, relative: HirRelativeId) -> Self {
        Self { family, relative }
    }

    pub(crate) const fn family(&self) -> &HirIdFamily {
        &self.family
    }

    pub(crate) const fn relative(&self) -> &HirRelativeId {
        &self.relative
    }
}

impl HirIdRef {
    pub(crate) const fn absolute(reference: HirEntityReference) -> Self {
        Self::Absolute(reference)
    }

    pub(crate) const fn relative(relative: HirRelativeId) -> Self {
        Self::Relative(relative)
    }

    pub(crate) const fn family_relative(relative: HirFamilyRelativeId) -> Self {
        Self::FamilyRelative(relative)
    }
}

impl HirCallArgumentOrdinal {
    pub(crate) fn try_new(value: usize) -> Result<Self, HirCallArgumentOrdinalError> {
        if value >= MAX_CALL_ARGUMENTS {
            return Err(HirCallArgumentOrdinalError {
                ordinal: value,
                limit: MAX_CALL_ARGUMENTS,
            });
        }
        let value = u16::try_from(value).map_err(|_| HirCallArgumentOrdinalError {
            ordinal: value,
            limit: MAX_CALL_ARGUMENTS,
        })?;
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> u16 {
        self.0
    }
}

impl HirDialogueCoordinate {
    pub(crate) const fn new(
        kind: HirDialogueCoordinateKind,
        argument: HirCallArgumentOrdinal,
        value: ExprId,
    ) -> Self {
        Self {
            kind,
            argument,
            value,
        }
    }

    pub(crate) const fn kind(&self) -> HirDialogueCoordinateKind {
        self.kind
    }

    pub(crate) const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    pub(crate) const fn value(&self) -> ExprId {
        self.value
    }
}

impl HirName {
    pub(crate) fn try_new(value: impl Into<Box<str>>) -> Result<Self, HirNameInvariantError> {
        let value = value.into();
        if !is_identifier(&value) {
            return Err(HirNameInvariantError { value });
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl HirLinePlan {
    pub(crate) fn try_new(
        root_scope: ScopeId,
        label: Option<HirName>,
        items: Box<[HirLinePlanItem]>,
    ) -> Result<Self, HirLinePlanInvariantError> {
        validate_line_plan_items(root_scope.module(), &items)?;
        Ok(Self {
            root_scope,
            label,
            items,
        })
    }

    pub(crate) const fn root_scope(&self) -> ScopeId {
        self.root_scope
    }

    pub(crate) const fn label(&self) -> Option<&HirName> {
        self.label.as_ref()
    }

    pub(crate) const fn items(&self) -> &[HirLinePlanItem] {
        &self.items
    }
}

fn validate_line_plan_items(
    expected: HirModuleId,
    items: &[HirLinePlanItem],
) -> Result<(), HirLinePlanInvariantError> {
    for item in items {
        match item {
            HirLinePlanItem::Init(statements) => {
                for statement in statements {
                    validate_line_plan_module(expected, statement.module())?;
                }
            }
            HirLinePlanItem::Thread(statement)
            | HirLinePlanItem::On(statement)
            | HirLinePlanItem::Statement(statement)
            | HirLinePlanItem::CancelRule(statement)
            | HirLinePlanItem::Error(statement) => {
                validate_line_plan_module(expected, statement.module())?;
            }
            HirLinePlanItem::Option { value, .. }
            | HirLinePlanItem::Out(value)
            | HirLinePlanItem::Expression(value)
            | HirLinePlanItem::TimelineAssert {
                condition: value, ..
            } => {
                validate_line_plan_module(expected, value.module())?;
            }
            HirLinePlanItem::Let { pattern, value } => {
                validate_line_plan_module(expected, pattern.module())?;
                validate_line_plan_module(expected, value.module())?;
            }
            HirLinePlanItem::TimedCue { anchor, body } => {
                validate_line_plan_module(expected, anchor.module())?;
                validate_line_plan_module(expected, body.module())?;
            }
            HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                validate_line_plan_items(expected, items)?;
            }
        }
    }
    Ok(())
}

fn validate_line_plan_module(
    expected: HirModuleId,
    actual: HirModuleId,
) -> Result<(), HirLinePlanInvariantError> {
    if actual == expected {
        Ok(())
    } else {
        Err(HirLinePlanInvariantError { expected, actual })
    }
}

impl HirPostfixCandidateFailure {
    pub(crate) const fn new(kind: HirPostfixCandidateFailureKind) -> Self {
        Self { kind }
    }

    pub(crate) const fn kind(&self) -> HirPostfixCandidateFailureKind {
        self.kind
    }
}

impl HirInsertionPoint {
    pub(crate) fn try_new(
        document: &SourceDocument,
        offset: usize,
    ) -> Result<Self, HirInsertionPointError> {
        let document_len = document.text().len();
        if offset > document_len {
            return Err(HirInsertionPointError::OutOfDocument {
                offset,
                document_len,
            });
        }
        if !document.text().is_char_boundary(offset) {
            return Err(HirInsertionPointError::NonUtf8Boundary { offset });
        }
        Ok(Self {
            source: document.identity().clone(),
            offset,
        })
    }

    pub(crate) const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub(crate) const fn offset(&self) -> usize {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HirCallArgumentOrdinal, HirCallArgumentOrdinalError, HirEntityReference,
        HirFamilyRelativeId, HirIdFamily, HirIdRef, HirIdRefInvariantError, HirIdSuffix,
        HirInsertionPoint, HirInsertionPointError, HirRelativeId,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    fn document(text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/hir-dialogue-application-test").expect("document ID"),
            SourceName::Generated,
            text,
        )
        .expect("test document")
    }

    #[test]
    fn ordinary_call_ordinal_reuses_the_existing_128_argument_limit() {
        assert_eq!(HirCallArgumentOrdinal::try_new(127).unwrap().get(), 127);
        assert_eq!(
            HirCallArgumentOrdinal::try_new(128),
            Err(HirCallArgumentOrdinalError {
                ordinal: 128,
                limit: 128,
            })
        );
    }

    #[test]
    fn typed_id_carriers_discard_authored_markers_but_preserve_structure() {
        assert_eq!(
            HirIdSuffix::try_new("@.next"),
            Err(HirIdRefInvariantError::AuthoredRelativeMarker)
        );
        assert_eq!(
            HirIdSuffix::try_new("chapter..opening"),
            Err(HirIdRefInvariantError::InvalidSuffix)
        );
        assert_eq!(
            HirIdFamily::try_new("not.a.family"),
            Err(HirIdRefInvariantError::InvalidFamily)
        );
        assert_eq!(
            HirEntityReference::try_new("flow.opening@sem:abc")
                .unwrap()
                .as_str(),
            "flow.opening@sem:abc"
        );

        let absolute = HirIdRef::absolute(HirEntityReference::try_new("character.alice").unwrap());
        let relative =
            HirIdRef::relative(HirRelativeId::new(HirIdSuffix::try_new("next").unwrap(), 0));
        let family_relative = HirIdRef::family_relative(HirFamilyRelativeId::new(
            HirIdFamily::try_new("flow").unwrap(),
            HirRelativeId::new(HirIdSuffix::try_new("next").unwrap(), 1),
        ));
        assert!(matches!(absolute, HirIdRef::Absolute(_)));
        assert!(matches!(relative, HirIdRef::Relative(_)));
        assert!(matches!(family_relative, HirIdRef::FamilyRelative(_)));
    }

    #[test]
    fn insertion_points_are_bound_to_exact_utf8_document_offsets() {
        let document = document("éx");
        let insertion = HirInsertionPoint::try_new(&document, 2).expect("after é");
        assert_eq!(insertion.offset(), 2);
        assert_eq!(insertion.source(), document.identity());
        assert_eq!(
            HirInsertionPoint::try_new(&document, 1),
            Err(HirInsertionPointError::NonUtf8Boundary { offset: 1 })
        );
        assert_eq!(
            HirInsertionPoint::try_new(&document, 4),
            Err(HirInsertionPointError::OutOfDocument {
                offset: 4,
                document_len: 3,
            })
        );
    }
}
