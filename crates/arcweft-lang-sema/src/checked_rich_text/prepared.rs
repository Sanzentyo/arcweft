//! Private pre-coordinate RichText evidence.
//!
//! The attribute checker runs before accepted-root coordinates can be issued.
//! Marker rows therefore remain HIR-qualified only in this private carrier;
//! the post-call seal consumes every row into the public checked model before
//! a `FinalSemanticAnalysis` can be published.

use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::DialogueHostEventKind;
use arcweft_lang_hir::{
    dialogue_application::{
        HirDialogueContentId, HirDialogueMarkId, HirDialogueMarkName, HirLineBreakKind,
        HirRichTextTagId,
    },
    identity::ExprId,
    source_index::HirSourceSite,
};
use arcweft_presentation::rich_text::{
    BuiltinRichTextFx, BuiltinRichTextFxPhase, RichTextDirectStyle, RichTextLayoutSelector,
    RichTextStyleSelector, RichTextTransformSelector,
};

use super::{
    CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDirectStyleSpan, CheckedLayoutSpan,
    CheckedObjectSpan, CheckedOwnerFields, CheckedRichTextClose, CheckedRichTextOwner,
    CheckedStyleSpan, CheckedTransformSpan, RichTextAttributeDiagnostic,
};

/// Exact content-qualified marker retained until accepted-root coordinates
/// exist. The diagnostic name is display-only and moves with the same row.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedDialogueMark {
    id: HirDialogueMarkId,
    diagnostic_name: HirDialogueMarkName,
}

/// Single affine inventory of the validated marker rows for one dialogue
/// content application. Cloneable expression shells retain only structural
/// marker positions; this catalog is moved through candidate transactions and
/// consumed exactly once by the post-call coordinate seal.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedDialogueMarkCatalog {
    content: HirDialogueContentId,
    rows: BTreeMap<HirRichTextTagId, PreparedCheckedDialogueMark>,
}

impl PreparedCheckedDialogueMarkCatalog {
    pub(crate) fn new(
        content: HirDialogueContentId,
        rows: BTreeMap<HirRichTextTagId, PreparedCheckedDialogueMark>,
    ) -> Self {
        Self { content, rows }
    }

    pub(crate) const fn content(&self) -> HirDialogueContentId {
        self.content
    }

    pub(crate) fn take(&mut self, tag: HirRichTextTagId) -> Option<PreparedCheckedDialogueMark> {
        self.rows.remove(&tag)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Affine checker result separating cloneable structural RichText preparation
/// from the sole marker-identity catalog.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedRichTextCheck {
    report: PreparedCheckedRichTextReport,
    markers: PreparedCheckedDialogueMarkCatalog,
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

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCheckedRichTextReport,
        PreparedCheckedDialogueMarkCatalog,
    ) {
        (self.report, self.markers)
    }
}

impl std::ops::Deref for PreparedCheckedRichTextCheck {
    type Target = PreparedCheckedRichTextReport;

    fn deref(&self) -> &Self::Target {
        &self.report
    }
}

impl PreparedCheckedDialogueMark {
    pub(crate) const fn new(id: HirDialogueMarkId, diagnostic_name: HirDialogueMarkName) -> Self {
        Self {
            id,
            diagnostic_name,
        }
    }

    #[cfg(test)]
    pub(crate) const fn id(&self) -> HirDialogueMarkId {
        self.id
    }

    pub(crate) fn into_parts(self) -> (HirDialogueMarkId, HirDialogueMarkName) {
        (self.id, self.diagnostic_name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCheckedRichTextAction {
    Control {
        action: CheckedDialogueControl,
        fields: CheckedOwnerFields,
    },
    DirectStyle {
        owner: RichTextDirectStyle,
        action: CheckedDirectStyleSpan,
        fields: CheckedOwnerFields,
    },
    Style {
        owner: RichTextStyleSelector,
        action: CheckedStyleSpan,
        fields: CheckedOwnerFields,
    },
    Layout {
        owner: RichTextLayoutSelector,
        action: CheckedLayoutSpan,
        fields: CheckedOwnerFields,
    },
    Transform {
        owner: RichTextTransformSelector,
        action: CheckedTransformSpan,
        fields: CheckedOwnerFields,
    },
    Object {
        action: CheckedObjectSpan,
        fields: CheckedOwnerFields,
    },
    BuiltinFx {
        effect: BuiltinRichTextFx,
        phase: BuiltinRichTextFxPhase,
        fields: CheckedOwnerFields,
    },
    Host {
        owner: DialogueHostEventKind,
        action: CheckedDialogueHostEvent,
        fields: CheckedOwnerFields,
    },
    /// Structural position only. The affine marker catalog owns the validated
    /// HIR identity and diagnostic name until the late coordinate seal.
    Marker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCheckedRichTextTag {
    id: HirRichTextTagId,
    owner: CheckedRichTextOwner,
    action: PreparedCheckedRichTextAction,
    source: HirSourceSite,
}

impl PreparedCheckedRichTextTag {
    pub(crate) const fn new(
        id: HirRichTextTagId,
        owner: CheckedRichTextOwner,
        action: PreparedCheckedRichTextAction,
        source: HirSourceSite,
    ) -> Self {
        Self {
            id,
            owner,
            action,
            source,
        }
    }

    pub(crate) const fn id(&self) -> HirRichTextTagId {
        self.id
    }

    #[cfg(test)]
    pub(crate) const fn owner(&self) -> &CheckedRichTextOwner {
        &self.owner
    }

    pub(crate) const fn action(&self) -> &PreparedCheckedRichTextAction {
        &self.action
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        HirRichTextTagId,
        CheckedRichTextOwner,
        PreparedCheckedRichTextAction,
        HirSourceSite,
    ) {
        (self.id, self.owner, self.action, self.source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCheckedDialogueToken {
    Text(Box<str>),
    RawText(Box<str>),
    Escape(char),
    Ruby {
        base: Box<str>,
        ruby: Box<str>,
    },
    Open(PreparedCheckedRichTextTag),
    Close(CheckedRichTextClose),
    InvalidTag {
        tag: HirRichTextTagId,
        source: HirSourceSite,
    },
    Interpolation(ExprId),
    LineBreak(HirLineBreakKind),
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
    diagnostics: Box<[RichTextAttributeDiagnostic]>,
}

impl PreparedCheckedRichTextReport {
    pub(crate) fn new(
        content: PreparedCheckedDialogueContent,
        diagnostics: Vec<RichTextAttributeDiagnostic>,
    ) -> Self {
        Self {
            content,
            diagnostics: diagnostics.into_boxed_slice(),
        }
    }

    pub(crate) const fn content(&self) -> &PreparedCheckedDialogueContent {
        &self.content
    }

    pub(crate) const fn diagnostics(&self) -> &[RichTextAttributeDiagnostic] {
        &self.diagnostics
    }

    pub(crate) const fn is_valid(&self) -> bool {
        self.content.diagnostics_complete && self.diagnostics.is_empty()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCheckedDialogueContent,
        Box<[RichTextAttributeDiagnostic]>,
    ) {
        (self.content, self.diagnostics)
    }
}
