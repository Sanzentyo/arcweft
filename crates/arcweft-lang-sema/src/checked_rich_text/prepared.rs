//! Private pre-coordinate evidence for checked dialogue content.
//!
//! Preparation retains typed point-action output and exact attached-content
//! node edges. Marker rows are kept in one affine catalog until accepted-root
//! coordinates can be issued; no source delimiter or close/open pairing is
//! represented here.

use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::DialogueHostEventKind;
use arcweft_lang_hir::dialogue_application::{
    HirDialogueContentId, HirDialogueMarkId, HirDialogueMarkName, HirDialogueNodeId,
    HirLineBreakKind,
};
use arcweft_lang_hir::identity::ExprId;

use super::{
    CheckedDialogueControl, CheckedDialogueHostEvent, CheckedOwnerFields, RichTextDiagnostic,
};

/// Exact content-qualified marker retained until accepted-root coordinates
/// exist. The diagnostic name is display-only and moves with the same row.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedDialogueMark {
    id: HirDialogueMarkId,
    diagnostic_name: HirDialogueMarkName,
}

/// Single affine inventory of validated marker rows for one content value.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedDialogueMarkCatalog {
    content: HirDialogueContentId,
    rows: BTreeMap<HirDialogueMarkId, PreparedCheckedDialogueMark>,
}

impl PreparedCheckedDialogueMarkCatalog {
    pub(crate) fn new(
        content: HirDialogueContentId,
        rows: BTreeMap<HirDialogueMarkId, PreparedCheckedDialogueMark>,
    ) -> Self {
        Self { content, rows }
    }

    pub(crate) const fn content(&self) -> HirDialogueContentId {
        self.content
    }

    pub(crate) fn take(&mut self, mark: HirDialogueMarkId) -> Option<PreparedCheckedDialogueMark> {
        self.rows.remove(&mark)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Affine checker result separating cloneable structure from marker identity.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedRichTextCheck {
    report: PreparedCheckedRichTextReport,
    markers: PreparedCheckedDialogueMarkCatalog,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedCheckedContentCatalogError {
    KeyMismatch {
        expected: HirDialogueContentId,
        actual: HirDialogueContentId,
        value: Box<PreparedCheckedRichTextCheck>,
    },
}

/// Sole affine inventory of prepared checked content rows awaiting late seal.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct PreparedCheckedContentCatalog {
    rows: BTreeMap<HirDialogueContentId, PreparedCheckedRichTextCheck>,
}

impl PreparedCheckedContentCatalog {
    pub(crate) fn insert(
        &mut self,
        value: PreparedCheckedRichTextCheck,
    ) -> Result<(), PreparedCheckedRichTextCheck> {
        let content = value.content_id();
        if value.markers.content != content || self.rows.contains_key(&content) {
            return Err(value);
        }
        self.rows.insert(content, value);
        Ok(())
    }

    pub(crate) fn take(
        &mut self,
        content: HirDialogueContentId,
    ) -> Option<PreparedCheckedRichTextCheck> {
        self.rows.remove(&content)
    }

    pub(crate) fn contains(&self, content: HirDialogueContentId) -> bool {
        self.rows.contains_key(&content)
    }

    pub(crate) fn replace(
        &mut self,
        content: HirDialogueContentId,
        value: Option<PreparedCheckedRichTextCheck>,
    ) -> Result<Option<PreparedCheckedRichTextCheck>, PreparedCheckedContentCatalogError> {
        match value {
            Some(value) if value.content_id() == content && value.markers.content() == content => {
                Ok(self.rows.insert(content, value))
            }
            Some(value) => Err(PreparedCheckedContentCatalogError::KeyMismatch {
                expected: content,
                actual: value.content_id(),
                value: Box::new(value),
            }),
            None => Ok(self.rows.remove(&content)),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl PreparedCheckedRichTextCheck {
    pub(crate) const fn new(
        report: PreparedCheckedRichTextReport,
        markers: PreparedCheckedDialogueMarkCatalog,
    ) -> Self {
        Self { report, markers }
    }

    pub(crate) const fn report(&self) -> &PreparedCheckedRichTextReport {
        &self.report
    }

    pub(crate) fn with_effect_plan(
        mut self,
        effect_plan: crate::final_analysis::PreparedDialogueEffectPlan,
    ) -> Self {
        self.report = self.report.with_effect_plan(effect_plan);
        self
    }

    pub(crate) const fn content_id(&self) -> HirDialogueContentId {
        self.report.content.id
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCheckedRichTextReport,
        PreparedCheckedDialogueMarkCatalog,
    ) {
        (self.report, self.markers)
    }
}

impl PreparedCheckedDialogueMark {
    pub(crate) const fn new(id: HirDialogueMarkId, diagnostic_name: HirDialogueMarkName) -> Self {
        Self {
            id,
            diagnostic_name,
        }
    }

    pub(crate) fn into_parts(self) -> (HirDialogueMarkId, HirDialogueMarkName) {
        (self.id, self.diagnostic_name)
    }
}

/// Prepared zero-width dialogue point action. Presentation modifiers are
/// intentionally absent; they are attached-content emissions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCheckedRichTextAction {
    Control {
        action: CheckedDialogueControl,
        fields: CheckedOwnerFields,
    },
    Host {
        owner: DialogueHostEventKind,
        action: CheckedDialogueHostEvent,
        fields: CheckedOwnerFields,
    },
    Marker {
        mark: HirDialogueMarkId,
    },
}

/// Structural reference to one checked attached-content child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedContentApplicationRef {
    node: HirDialogueNodeId,
    expression: ExprId,
}

impl PreparedContentApplicationRef {
    pub(crate) const fn new(node: HirDialogueNodeId, expression: ExprId) -> Self {
        Self { node, expression }
    }

    pub(crate) const fn node(&self) -> HirDialogueNodeId {
        self.node
    }

    pub(crate) const fn expression(&self) -> ExprId {
        self.expression
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCheckedDialogueToken {
    Text(Box<str>),
    Escape(char),
    Interpolation(ExprId),
    PointAction(PreparedCheckedRichTextAction),
    ContentApplication(PreparedContentApplicationRef),
    LineBreak(HirLineBreakKind),
    /// Typed raw literal. It has no child nodes and is never reparsed.
    RawLiteral(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedDialogueContent {
    id: HirDialogueContentId,
    tokens: Box<[PreparedCheckedDialogueToken]>,
    diagnostics_complete: bool,
}

impl PreparedCheckedDialogueContent {
    pub(crate) fn new(
        id: HirDialogueContentId,
        tokens: Vec<PreparedCheckedDialogueToken>,
        diagnostics_complete: bool,
    ) -> Self {
        Self {
            id,
            tokens: tokens.into_boxed_slice(),
            diagnostics_complete,
        }
    }

    pub(crate) const fn tokens(&self) -> &[PreparedCheckedDialogueToken] {
        &self.tokens
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        HirDialogueContentId,
        Box<[PreparedCheckedDialogueToken]>,
        bool,
    ) {
        (self.id, self.tokens, self.diagnostics_complete)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedRichTextReport {
    content: PreparedCheckedDialogueContent,
    diagnostics: Box<[RichTextDiagnostic]>,
    effect_plan: crate::final_analysis::PreparedDialogueEffectPlan,
}

impl PreparedCheckedRichTextReport {
    pub(crate) fn new(
        content: PreparedCheckedDialogueContent,
        diagnostics: Vec<RichTextDiagnostic>,
    ) -> Self {
        Self {
            content,
            diagnostics: diagnostics.into_boxed_slice(),
            effect_plan: crate::final_analysis::PreparedDialogueEffectPlan::new([]),
        }
    }

    pub(crate) fn with_effect_plan(
        mut self,
        effect_plan: crate::final_analysis::PreparedDialogueEffectPlan,
    ) -> Self {
        self.effect_plan = effect_plan;
        self
    }

    pub(crate) const fn content(&self) -> &PreparedCheckedDialogueContent {
        &self.content
    }

    pub(crate) const fn diagnostics(&self) -> &[RichTextDiagnostic] {
        &self.diagnostics
    }

    pub(crate) const fn is_valid(&self) -> bool {
        self.content.diagnostics_complete && self.diagnostics.is_empty()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCheckedDialogueContent,
        Box<[RichTextDiagnostic]>,
        crate::final_analysis::PreparedDialogueEffectPlan,
    ) {
        (self.content, self.diagnostics, self.effect_plan)
    }
}
