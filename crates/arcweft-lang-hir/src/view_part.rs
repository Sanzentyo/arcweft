//! HIR catalog for private View-part targets and authored exports.

use arcweft_lang_syntax::ast::{
    common::{TextRange, Visibility},
    ids::EntityRef,
    items::EntityDeclItem,
    module_path::CanonicalModulePath,
    view::{ViewBody, ViewExpr, ViewModifier, ViewPartExportDecl, ViewPartModifier},
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use std::sync::Arc;

/// One View definition and all part declarations it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirViewPartOwner {
    view: EntityRef,
    module: Option<CanonicalModulePath>,
    visibility: Option<Visibility>,
    range: TextRange,
    source: Arc<SourceDocumentIdentity>,
    local_parts: Vec<HirViewLocalPart>,
    exports: Vec<HirViewPartExport>,
}

/// One private local part attached to a static node-producing expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirViewLocalPart {
    name: String,
    modifier: SourceSpan,
    operand: SourceSpan,
    target_range: TextRange,
    target_kind: HirViewPartTargetKind,
    occurrence: HirViewPartOccurrenceShape,
}

/// One leading authored public export declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirViewPartExport {
    local_name: String,
    public_name: String,
    declaration: SourceSpan,
    local_operand: SourceSpan,
    public_operand: SourceSpan,
}

/// Syntax/HIR target family before product instruction projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirViewPartTargetKind {
    Element,
    Text,
    Image,
    ViewCall,
}

/// Whether one static target can be omitted or repeated at runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HirViewPartOccurrenceShape {
    conditional_depth: u32,
    repeat_depth: u32,
}

impl HirViewPartOwner {
    pub(crate) fn from_syntax(
        module: Option<CanonicalModulePath>,
        item: &EntityDeclItem,
        body: &ViewBody,
    ) -> Option<Self> {
        let mut local_parts = Vec::new();
        collect_local_parts(
            body.value(),
            HirViewPartOccurrenceShape::default(),
            &mut local_parts,
        );
        let exports = body
            .exports()
            .iter()
            .map(HirViewPartExport::from)
            .collect::<Vec<_>>();
        let source = local_parts
            .first()
            .map(|part: &HirViewLocalPart| part.modifier.source_identity())
            .or_else(|| {
                exports
                    .first()
                    .map(|export| export.declaration.source_identity())
            })?;
        Some(Self {
            view: item.id().clone(),
            module,
            visibility: item.visibility(),
            range: body.range(),
            source,
            local_parts,
            exports,
        })
    }

    pub const fn view(&self) -> &EntityRef {
        &self.view
    }

    pub const fn module(&self) -> Option<&CanonicalModulePath> {
        self.module.as_ref()
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }

    pub const fn source(&self) -> &Arc<SourceDocumentIdentity> {
        &self.source
    }

    pub fn local_parts(&self) -> &[HirViewLocalPart] {
        &self.local_parts
    }

    pub fn exports(&self) -> &[HirViewPartExport] {
        &self.exports
    }

    pub(crate) fn assign_module(&mut self, module: &CanonicalModulePath) {
        self.module = Some(module.clone());
    }
}

impl HirViewLocalPart {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn modifier_span(&self) -> &SourceSpan {
        &self.modifier
    }

    pub const fn operand_span(&self) -> &SourceSpan {
        &self.operand
    }

    pub const fn target_range(&self) -> TextRange {
        self.target_range
    }

    pub const fn target_kind(&self) -> HirViewPartTargetKind {
        self.target_kind
    }

    pub const fn occurrence(&self) -> HirViewPartOccurrenceShape {
        self.occurrence
    }
}

impl HirViewPartExport {
    pub fn local_name(&self) -> &str {
        &self.local_name
    }

    pub fn public_name(&self) -> &str {
        &self.public_name
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration
    }

    pub const fn local_operand_span(&self) -> &SourceSpan {
        &self.local_operand
    }

    pub const fn public_operand_span(&self) -> &SourceSpan {
        &self.public_operand
    }
}

impl HirViewPartOccurrenceShape {
    pub const fn can_be_absent(self) -> bool {
        self.conditional_depth != 0 || self.repeat_depth != 0
    }

    pub const fn can_repeat(self) -> bool {
        self.repeat_depth != 0
    }

    const fn conditional(self) -> Self {
        Self {
            conditional_depth: self.conditional_depth.saturating_add(1),
            repeat_depth: self.repeat_depth,
        }
    }

    const fn repeated(self) -> Self {
        Self {
            conditional_depth: self.conditional_depth,
            repeat_depth: self.repeat_depth.saturating_add(1),
        }
    }
}

impl From<&ViewPartExportDecl> for HirViewPartExport {
    fn from(declaration: &ViewPartExportDecl) -> Self {
        Self {
            local_name: declaration.local_name().text().to_owned(),
            public_name: declaration.public_name().text().to_owned(),
            declaration: declaration.declaration_span().clone(),
            local_operand: declaration.local_operand_span().clone(),
            public_operand: declaration.public_operand_span().clone(),
        }
    }
}

fn collect_local_parts(
    expression: &ViewExpr,
    occurrence: HirViewPartOccurrenceShape,
    output: &mut Vec<HirViewLocalPart>,
) {
    match expression {
        ViewExpr::Fragment(children) => children
            .iter()
            .for_each(|child| collect_local_parts(child, occurrence, output)),
        ViewExpr::Element(element) => {
            collect_target(
                element.modifiers(),
                element.range(),
                HirViewPartTargetKind::Element,
                occurrence,
                output,
            );
            element
                .children()
                .iter()
                .for_each(|child| collect_local_parts(child, occurrence, output));
        }
        ViewExpr::ViewCall(call) => collect_target(
            call.modifiers(),
            call.range(),
            HirViewPartTargetKind::ViewCall,
            occurrence,
            output,
        ),
        ViewExpr::Text(text) => collect_target(
            text.modifiers(),
            text.range(),
            HirViewPartTargetKind::Text,
            occurrence,
            output,
        ),
        ViewExpr::Image(image) => collect_target(
            image.modifiers(),
            image.range(),
            HirViewPartTargetKind::Image,
            occurrence,
            output,
        ),
        ViewExpr::TextField(field) => collect_target(
            field.modifiers(),
            field.range(),
            HirViewPartTargetKind::Element,
            occurrence,
            output,
        ),
        ViewExpr::Button(button) => collect_target(
            button.modifiers(),
            button.range(),
            HirViewPartTargetKind::Element,
            occurrence,
            output,
        ),
        ViewExpr::If(branch) => {
            let occurrence = occurrence.conditional();
            collect_local_parts(branch.then_branch(), occurrence, output);
            branch
                .else_branch()
                .into_iter()
                .for_each(|child| collect_local_parts(child, occurrence, output));
        }
        ViewExpr::Match(view_match) => {
            let occurrence = occurrence.conditional();
            view_match
                .arms()
                .iter()
                .for_each(|arm| collect_local_parts(arm.value(), occurrence, output));
        }
        ViewExpr::ForEach(view_for_each) => {
            collect_local_parts(view_for_each.body(), occurrence.repeated(), output);
        }
        ViewExpr::Await(view_await) => {
            let occurrence = occurrence.conditional();
            view_await
                .branches()
                .iter()
                .for_each(|branch| collect_local_parts(branch.value(), occurrence, output));
        }
        ViewExpr::Let(_) | ViewExpr::Expr(_) | ViewExpr::Raw(_) => {}
    }
}

fn collect_target(
    modifiers: &[ViewModifier],
    target_range: TextRange,
    target_kind: HirViewPartTargetKind,
    occurrence: HirViewPartOccurrenceShape,
    output: &mut Vec<HirViewLocalPart>,
) {
    output.extend(modifiers.iter().filter_map(|modifier| {
        let ViewModifier::Part(label) = modifier else {
            return None;
        };
        Some(local_part(label, target_range, target_kind, occurrence))
    }));
}

fn local_part(
    label: &ViewPartModifier,
    target_range: TextRange,
    target_kind: HirViewPartTargetKind,
    occurrence: HirViewPartOccurrenceShape,
) -> HirViewLocalPart {
    HirViewLocalPart {
        name: label.local_name().text().to_owned(),
        modifier: label.modifier_span().clone(),
        operand: label.operand_span().clone(),
        target_range,
        target_kind,
        occurrence,
    }
}
