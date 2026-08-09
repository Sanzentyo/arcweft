//! Typed View declaration ownership over the attached grammar tree.

use arcweft_source::{SourceRange, SourceSpan};

use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::grammar::view_projection::{PendingViewPartLocalName, PendingViewRequiredKeyword};
use crate::patterns::PatternSyntaxFamily;

use super::node::{
    AstNode, CloseBraceKind, DeclarationHeaderKind, ErrorNodeKind, FixedParameterGroupKind,
    MissingBodyKind, MissingNameKind, MissingTokenNodeKind, OpenBraceKind, PathKind,
    ViewDeclarationBodyKind, ViewDeclarationItemKind, ViewExportBlockKind,
    ViewExportDeclarationKind, ViewFragmentKind,
};
use super::source_file::AttachedPath;
use super::{
    AttachedCallableParameter, AttachedDeclarationPublicId, AttachedExpressionNode,
    AttachedFixedParameterGroup, AttachedItemPrefix, AttachedRetainedHeader, AttachedRetainedName,
    SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};

/// An authored required View-export keyword or its typed insertion owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedViewRequiredKeyword {
    Authored(SourceSpan),
    Missing {
        source: SourceSpan,
        syntax: AstNode<MissingTokenNodeKind>,
    },
}

impl AttachedViewRequiredKeyword {
    pub const fn source_span(&self) -> &SourceSpan {
        match self {
            Self::Authored(source) | Self::Missing { source, .. } => source,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// One local or public View part path, including exact missing-name recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedViewPartPath {
    Path(AttachedPath),
    Missing(AstNode<MissingNameKind>),
}

impl AttachedViewPartPath {
    pub const fn path(&self) -> Option<&AttachedPath> {
        match self {
            Self::Path(path) => Some(path),
            Self::Missing(_) => None,
        }
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingNameKind>> {
        match self {
            Self::Path(_) => None,
            Self::Missing(syntax) => Some(syntax),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Path(path) => path.missing_name().is_some(),
            Self::Missing(_) => true,
        }
    }
}

/// One source-ordered View part export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedViewExport {
    syntax: AstNode<ViewExportDeclarationKind>,
    source_ordinal: u16,
    part: AttachedViewRequiredKeyword,
    local_part: AttachedViewPartPath,
    alias: AttachedViewRequiredKeyword,
    public_part: AttachedViewPartPath,
    misplaced: bool,
}

impl AttachedViewExport {
    pub const fn syntax(&self) -> &AstNode<ViewExportDeclarationKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn part(&self) -> &AttachedViewRequiredKeyword {
        &self.part
    }

    pub const fn local_part(&self) -> &AttachedViewPartPath {
        &self.local_part
    }

    pub const fn alias(&self) -> &AttachedViewRequiredKeyword {
        &self.alias
    }

    pub const fn public_part(&self) -> &AttachedViewPartPath {
        &self.public_part
    }

    pub const fn is_misplaced(&self) -> bool {
        self.misplaced
    }

    pub fn has_recovery(&self) -> bool {
        self.part.is_missing()
            || self.local_part.has_recovery()
            || self.alias.is_missing()
            || self.public_part.has_recovery()
            || self.misplaced
    }
}

/// One source-interleaved View fragment entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedViewFragmentEntry {
    Value(AttachedExpressionNode),
    MisplacedExport(AttachedViewExport),
}

/// Typed local-name state for one parser-owned View `.part(...)` modifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedViewPartLocalName {
    Present(SourceSpan),
    Missing(SourceSpan),
    Invalid(SourceSpan),
}

impl AttachedViewPartLocalName {
    pub const fn source_span(&self) -> &SourceSpan {
        match self {
            Self::Present(source) | Self::Missing(source) | Self::Invalid(source) => source,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Present(_))
    }
}

/// Exact source-role projection for one View `.part(...)` modifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedViewPartModifier {
    source_ordinal: u32,
    whole: SourceSpan,
    dot: SourceSpan,
    name: SourceSpan,
    open: SourceSpan,
    local_name: AttachedViewPartLocalName,
    close: Option<SourceSpan>,
}

impl AttachedViewPartModifier {
    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }

    pub const fn dot(&self) -> &SourceSpan {
        &self.dot
    }

    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }

    pub const fn open(&self) -> &SourceSpan {
        &self.open
    }

    pub const fn local_name(&self) -> &AttachedViewPartLocalName {
        &self.local_name
    }

    pub const fn close(&self) -> Option<&SourceSpan> {
        self.close.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        self.local_name.has_recovery() || self.close.is_none()
    }
}

impl AttachedViewFragmentEntry {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Value(value) => value.projection().has_recovery(),
            Self::MisplacedExport(export) => export.has_recovery(),
        }
    }
}

/// The required View value fragment with exact value/export source interleaving.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedViewFragment {
    syntax: AstNode<ViewFragmentKind>,
    entries: Box<[AttachedViewFragmentEntry]>,
    part_modifiers: Box<[AttachedViewPartModifier]>,
}

impl AttachedViewFragment {
    pub const fn syntax(&self) -> &AstNode<ViewFragmentKind> {
        &self.syntax
    }

    pub const fn entries(&self) -> &[AttachedViewFragmentEntry] {
        &self.entries
    }

    pub const fn part_modifiers(&self) -> &[AttachedViewPartModifier] {
        &self.part_modifiers
    }

    pub fn values(&self) -> impl Iterator<Item = &AttachedExpressionNode> {
        self.entries.iter().filter_map(|entry| match entry {
            AttachedViewFragmentEntry::Value(value) => Some(value),
            AttachedViewFragmentEntry::MisplacedExport(_) => None,
        })
    }

    pub fn misplaced_exports(&self) -> impl Iterator<Item = &AttachedViewExport> {
        self.entries.iter().filter_map(|entry| match entry {
            AttachedViewFragmentEntry::Value(_) => None,
            AttachedViewFragmentEntry::MisplacedExport(export) => Some(export),
        })
    }

    pub fn has_recovery(&self) -> bool {
        self.entries
            .iter()
            .any(AttachedViewFragmentEntry::has_recovery)
            || self
                .part_modifiers
                .iter()
                .any(AttachedViewPartModifier::has_recovery)
    }
}

/// Missing or authored View body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedViewBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<ViewDeclarationBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        leading_exports: Box<[AttachedViewExport]>,
        fragment: AttachedViewFragment,
    },
}

impl AttachedViewBody {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Missing(syntax) => syntax.syntax(),
            Self::Braced { syntax, .. } => syntax.syntax(),
        }
    }

    pub const fn leading_exports(&self) -> &[AttachedViewExport] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced {
                leading_exports, ..
            } => leading_exports,
        }
    }

    pub const fn fragment(&self) -> Option<&AttachedViewFragment> {
        match self {
            Self::Missing(_) => None,
            Self::Braced { fragment, .. } => Some(fragment),
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unclosed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if close.range().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        self.is_missing()
            || self.is_unclosed()
            || self
                .leading_exports()
                .iter()
                .any(AttachedViewExport::has_recovery)
            || self
                .fragment()
                .is_some_and(AttachedViewFragment::has_recovery)
    }
}

/// One source-bound retained View declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedViewDeclaration {
    syntax: AstNode<ViewDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    parameter_group: AttachedFixedParameterGroup,
    body: AttachedViewBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedViewDeclaration {
    pub const fn syntax(&self) -> &AstNode<ViewDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn parameter_group(&self) -> &AttachedFixedParameterGroup {
        &self.parameter_group
    }

    pub const fn body(&self) -> &AttachedViewBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn exports(&self) -> impl Iterator<Item = &AttachedViewExport> {
        self.body.leading_exports().iter().chain(
            self.body
                .fragment()
                .into_iter()
                .flat_map(AttachedViewFragment::misplaced_exports),
        )
    }

    pub fn has_recovery(&self) -> bool {
        retained_header_has_recovery(&self.header)
            || self
                .parameter_group
                .parameters()
                .iter()
                .any(view_parameter_has_recovery)
            || self.parameter_group.has_recovery()
            || self.body.has_recovery()
            || self.header_recovery.is_some()
            || self.trailing_recovery.is_some()
    }
}

impl AstNode<ViewDeclarationItemKind> {
    /// Binds the one-pass View grammar without source or diagnostic rediscovery.
    pub fn semantics(&self) -> Result<AttachedViewDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::View(self.clone());
        let header_syntax =
            self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let mut next_parameter_ordinal = 0;
        let parameter_group = header_syntax
            .required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)?
            .callable_semantics(0, &mut next_parameter_ordinal)?;
        let body = attach_body(self)?;
        let body_range = body.syntax().range();
        let mut header_recovery = None;
        let mut trailing_recovery = None;
        for recovery in self.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)? {
            if recovery.range().end() <= body_range.start() && !recovery.range().is_empty() {
                if header_recovery.replace(recovery).is_some() {
                    return Err(SyntaxAccessError::InvalidViewProjection { id: self.id() });
                }
            } else if recovery.range().start() >= body_range.end() {
                if trailing_recovery.replace(recovery).is_some() {
                    return Err(SyntaxAccessError::InvalidViewProjection { id: self.id() });
                }
            } else {
                return Err(SyntaxAccessError::InvalidViewProjection { id: self.id() });
            }
        }
        Ok(AttachedViewDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header_syntax.retained_semantics()?,
            parameter_group,
            body,
            header_recovery,
            trailing_recovery,
        })
    }
}

fn attach_body(
    owner: &AstNode<ViewDeclarationItemKind>,
) -> Result<AttachedViewBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidViewProjection { id: owner.id() })?;
    match body.kind() {
        SyntaxKind::MissingBody => Ok(AttachedViewBody::Missing(body.cast()?)),
        SyntaxKind::ViewDeclarationBody => {
            let syntax = body.cast::<ViewDeclarationBodyKind>()?;
            let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            let export_block =
                syntax.optional_exact_child::<ViewExportBlockKind>(SyntaxRole::Element(0))?;
            let mut next_export_ordinal = 0;
            let leading_exports = export_block
                .as_ref()
                .map(|block| attach_leading_exports(block, &mut next_export_ordinal))
                .transpose()?
                .unwrap_or_default();
            let fragment = attach_fragment(
                syntax.required_exact_child::<ViewFragmentKind>(SyntaxRole::Tail)?,
                &mut next_export_ordinal,
            )?;
            Ok(AttachedViewBody::Braced {
                syntax,
                open,
                close,
                leading_exports,
                fragment,
            })
        }
        _ => Err(SyntaxAccessError::InvalidViewProjection { id: owner.id() }),
    }
}

fn attach_leading_exports(
    block: &AstNode<ViewExportBlockKind>,
    next_export_ordinal: &mut u16,
) -> Result<Box<[AttachedViewExport]>, SyntaxAccessError> {
    let children =
        block.ordered_exact_children::<ViewExportDeclarationKind>(SyntaxRoleClass::Export)?;
    if children.len() != block.syntax().children().len() {
        return Err(SyntaxAccessError::InvalidViewProjection { id: block.id() });
    }
    children
        .into_iter()
        .enumerate()
        .map(|(local_ordinal, syntax)| {
            let local_ordinal = u16::try_from(local_ordinal)
                .map_err(|_| SyntaxAccessError::InvalidViewProjection { id: block.id() })?;
            let source_ordinal = take_export_ordinal(next_export_ordinal, block.id())?;
            attach_export(syntax, source_ordinal, local_ordinal, false)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn attach_fragment(
    syntax: AstNode<ViewFragmentKind>,
    next_export_ordinal: &mut u16,
) -> Result<AttachedViewFragment, SyntaxAccessError> {
    let projection = syntax
        .syntax()
        .view_fragment_projection()
        .cloned()
        .ok_or(SyntaxAccessError::MissingViewFragmentProjection { id: syntax.id() })?;
    let part_modifiers = projection
        .part_modifiers()
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, modifier)| {
            let source_ordinal = u32::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidViewProjection { id: syntax.id() })?;
            Ok(AttachedViewPartModifier {
                source_ordinal,
                whole: syntax.syntax().source_span_for_range(modifier.whole()),
                dot: syntax.syntax().source_span_for_range(modifier.dot()),
                name: syntax.syntax().source_span_for_range(modifier.name()),
                open: syntax.syntax().source_span_for_range(modifier.open()),
                local_name: match modifier.local_name() {
                    PendingViewPartLocalName::Present(source) => {
                        AttachedViewPartLocalName::Present(
                            syntax.syntax().source_span_for_range(source),
                        )
                    }
                    PendingViewPartLocalName::Missing(source) => {
                        AttachedViewPartLocalName::Missing(
                            syntax.syntax().source_span_for_range(source),
                        )
                    }
                    PendingViewPartLocalName::Invalid(source) => {
                        AttachedViewPartLocalName::Invalid(
                            syntax.syntax().source_span_for_range(source),
                        )
                    }
                },
                close: modifier
                    .close()
                    .map(|source| syntax.syntax().source_span_for_range(source)),
            })
        })
        .collect::<Result<Vec<_>, SyntaxAccessError>>()?
        .into_boxed_slice();
    let mut next_value_ordinal = 0_u32;
    let mut next_misplaced_ordinal = 0_u16;
    let mut entries = Vec::new();
    let mut previous_end = syntax.range().start();
    for child in syntax.syntax().children() {
        if child.range().start() < previous_end {
            return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
        }
        previous_end = child.range().end();
        if child.kind() == SyntaxKind::ViewExportDeclaration {
            if child.role() != SyntaxRole::Export(next_misplaced_ordinal) {
                return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
            }
            let source_ordinal = take_export_ordinal(next_export_ordinal, syntax.id())?;
            entries.push(AttachedViewFragmentEntry::MisplacedExport(attach_export(
                child.cast()?,
                source_ordinal,
                next_misplaced_ordinal,
                true,
            )?));
            next_misplaced_ordinal = next_misplaced_ordinal
                .checked_add(1)
                .ok_or(SyntaxAccessError::InvalidViewProjection { id: syntax.id() })?;
        } else if child.expression_projection().is_some() {
            if child.role() != SyntaxRole::Element(next_value_ordinal) {
                return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
            }
            entries.push(AttachedViewFragmentEntry::Value(
                AttachedExpressionNode::from_syntax(child)?,
            ));
            next_value_ordinal = next_value_ordinal
                .checked_add(1)
                .ok_or(SyntaxAccessError::InvalidViewProjection { id: syntax.id() })?;
        } else {
            return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
        }
    }
    Ok(AttachedViewFragment {
        syntax,
        entries: entries.into_boxed_slice(),
        part_modifiers,
    })
}

fn attach_export(
    syntax: AstNode<ViewExportDeclarationKind>,
    source_ordinal: u16,
    local_ordinal: u16,
    misplaced: bool,
) -> Result<AttachedViewExport, SyntaxAccessError> {
    if syntax.role() != SyntaxRole::Export(local_ordinal) {
        return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
    }
    let pending = syntax
        .syntax()
        .view_export_projection()
        .cloned()
        .ok_or(SyntaxAccessError::MissingViewExportProjection { id: syntax.id() })?;
    if pending.is_misplaced() != misplaced {
        return Err(SyntaxAccessError::InvalidViewProjection { id: syntax.id() });
    }
    Ok(AttachedViewExport {
        part: attach_keyword(&syntax, pending.part(), SyntaxRole::Kind)?,
        local_part: attach_part_path(&syntax, SyntaxRole::Target)?,
        alias: attach_keyword(&syntax, pending.alias(), SyntaxRole::Alias)?,
        public_part: attach_part_path(&syntax, SyntaxRole::Name)?,
        syntax,
        source_ordinal,
        misplaced,
    })
}

fn attach_keyword(
    owner: &AstNode<ViewExportDeclarationKind>,
    pending: PendingViewRequiredKeyword,
    role: SyntaxRole,
) -> Result<AttachedViewRequiredKeyword, SyntaxAccessError> {
    let source = pending.source();
    if !range_belongs_to(owner.range(), source) || pending.is_missing() != source.is_empty() {
        return Err(SyntaxAccessError::InvalidViewProjection { id: owner.id() });
    }
    let missing = owner.optional_exact_child::<MissingTokenNodeKind>(role)?;
    match (pending, missing) {
        (PendingViewRequiredKeyword::Authored(_), None) => Ok(
            AttachedViewRequiredKeyword::Authored(owner.syntax().source_span_for_range(source)),
        ),
        (PendingViewRequiredKeyword::Missing(_), Some(syntax)) if syntax.range() == source => {
            Ok(AttachedViewRequiredKeyword::Missing {
                source: owner.syntax().source_span_for_range(source),
                syntax,
            })
        }
        _ => Err(SyntaxAccessError::InvalidViewProjection { id: owner.id() }),
    }
}

fn attach_part_path(
    owner: &AstNode<ViewExportDeclarationKind>,
    role: SyntaxRole,
) -> Result<AttachedViewPartPath, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidViewProjection { id: owner.id() })?;
    match syntax.kind() {
        SyntaxKind::Path => Ok(AttachedViewPartPath::Path(AttachedPath::from_syntax(
            syntax.cast::<PathKind>()?,
        )?)),
        SyntaxKind::MissingName => Ok(AttachedViewPartPath::Missing(syntax.cast()?)),
        _ => Err(SyntaxAccessError::InvalidViewProjection { id: owner.id() }),
    }
}

fn take_export_ordinal(
    next: &mut u16,
    owner: super::SyntaxNodeId,
) -> Result<u16, SyntaxAccessError> {
    let ordinal = *next;
    *next = next
        .checked_add(1)
        .ok_or(SyntaxAccessError::InvalidViewProjection { id: owner })?;
    Ok(ordinal)
}

fn retained_header_has_recovery(header: &AttachedRetainedHeader) -> bool {
    matches!(
        header.public_id(),
        AttachedDeclarationPublicId::Recovered { .. }
    ) || !matches!(header.name(), AttachedRetainedName::Resolved { .. })
}

fn view_parameter_has_recovery(parameter: &AttachedCallableParameter) -> bool {
    parameter.has_recovery()
        || parameter.is_rest()
        || parameter.pattern().family() != PatternSyntaxFamily::Binding
}

const fn range_belongs_to(owner: SourceRange, child: SourceRange) -> bool {
    owner.start() <= child.start() && child.end() <= owner.end()
}

#[cfg(test)]
mod tests;
