//! Canonical, syntax-owned edits for authored View-part identities.

use arcweft_lang_syntax::{
    ast::{
        items::Item,
        view::{ViewExpr, ViewModifier},
    },
    source::ParsedSource,
};

use crate::model::TextEdit;

/// Returns non-overlapping edits only for fully parsed View-part syntax.
///
/// Malformed and misplaced declarations are absent from the typed export list,
/// so their source remains byte-for-byte unchanged while valid surrounding
/// declarations and modifiers can still be normalized.
pub(super) fn canonical_edits(source: &str, parsed: &ParsedSource) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for item in parsed.typed_tree().items() {
        let Item::EntityDecl(entity) = item else {
            continue;
        };
        let Some(body) = entity.view_body() else {
            continue;
        };
        let Some(view) = body.view() else {
            continue;
        };

        for declaration in view.exports() {
            let range = declaration.declaration_span().range();
            let replacement = format!(
                "export part {} as {}",
                declaration.local_name().text(),
                declaration.public_name().text()
            );
            push_if_changed(source, range.start(), range.end(), replacement, &mut edits);
        }
        collect_part_edits(source, view.value(), &mut edits);
    }
    edits
}

fn collect_part_edits(source: &str, expression: &ViewExpr, edits: &mut Vec<TextEdit>) {
    match expression {
        ViewExpr::Fragment(items) => {
            for item in items {
                collect_part_edits(source, item, edits);
            }
        }
        ViewExpr::Element(element) => {
            collect_modifier_edits(source, element.modifiers(), edits);
            for child in element.children() {
                collect_part_edits(source, child, edits);
            }
        }
        ViewExpr::ViewCall(call) => collect_modifier_edits(source, call.modifiers(), edits),
        ViewExpr::Text(text) => collect_modifier_edits(source, text.modifiers(), edits),
        ViewExpr::Image(image) => collect_modifier_edits(source, image.modifiers(), edits),
        ViewExpr::TextField(field) => collect_modifier_edits(source, field.modifiers(), edits),
        ViewExpr::Button(button) => collect_modifier_edits(source, button.modifiers(), edits),
        ViewExpr::If(branch) => {
            collect_part_edits(source, branch.then_branch(), edits);
            if let Some(otherwise) = branch.else_branch() {
                collect_part_edits(source, otherwise, edits);
            }
        }
        ViewExpr::Match(branch) => {
            for arm in branch.arms() {
                collect_part_edits(source, arm.value(), edits);
            }
        }
        ViewExpr::ForEach(repeat) => collect_part_edits(source, repeat.body(), edits),
        ViewExpr::Await(awaited) => {
            for branch in awaited.branches() {
                collect_part_edits(source, branch.value(), edits);
            }
        }
        ViewExpr::Let(_) | ViewExpr::Expr(_) | ViewExpr::Raw(_) => {}
    }
}

fn collect_modifier_edits(source: &str, modifiers: &[ViewModifier], edits: &mut Vec<TextEdit>) {
    for modifier in modifiers {
        if let ViewModifier::Part(part) = modifier {
            let range = part.modifier_span().range();
            push_if_changed(
                source,
                range.start(),
                range.end(),
                format!(".part({})", part.local_name().text()),
                edits,
            );
        }
    }
}

fn push_if_changed(
    source: &str,
    start: usize,
    end: usize,
    replacement: String,
    edits: &mut Vec<TextEdit>,
) {
    if source
        .get(start..end)
        .is_some_and(|authored| authored != replacement)
    {
        edits.push(TextEdit {
            start,
            end,
            replacement,
        });
    }
}
