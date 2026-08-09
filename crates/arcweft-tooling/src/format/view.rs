//! Canonical, syntax-owned edits for authored View-part identities.

use arcweft_lang_syntax::attachment::{
    AttachedPath, AttachedPathRoot, AttachedViewPartLocalName, AttachedViewPartModifier,
    AttachedViewPartPath, SyntaxAccessError, TypedItemNode, source_file::AttachedPathSegment,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::model::TextEdit;

/// Returns non-overlapping edits only for fully attached View-part syntax.
pub(super) fn canonical_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Result<Vec<TextEdit>, SyntaxAccessError> {
    let mut edits = Vec::new();
    for item in parsed.items()? {
        let TypedItemNode::View(view) = item else {
            continue;
        };
        let view = view.semantics()?;
        for export in view.exports().filter(|export| !export.has_recovery()) {
            let (Some(local_part), Some(public_part)) = (
                canonical_part_path(export.local_part()),
                canonical_part_path(export.public_part()),
            ) else {
                continue;
            };
            let range = export.syntax().range();
            push_if_changed(
                source,
                range.start(),
                range.end(),
                format!("export part {local_part} as {public_part}"),
                &mut edits,
            );
        }
        if let Some(fragment) = view.body().fragment() {
            for modifier in fragment.part_modifiers() {
                if let Some(edit) = canonical_part_modifier(source, modifier) {
                    edits.push(edit);
                }
            }
        }
    }
    Ok(edits)
}

fn canonical_part_path(path: &AttachedViewPartPath) -> Option<String> {
    let AttachedViewPartPath::Path(path) = path else {
        return None;
    };
    (!path.has_recovery()).then(|| canonical_path(path))
}

fn canonical_path(path: &AttachedPath) -> String {
    let mut parts = Vec::new();
    match path.root() {
        AttachedPathRoot::ImplicitCrate => {}
        AttachedPathRoot::Crate { .. } => parts.push("crate"),
        AttachedPathRoot::SelfModule { .. } => parts.push("self"),
        AttachedPathRoot::Super { levels } => {
            parts.extend(std::iter::repeat_n("super", levels.len()));
        }
    }
    parts.extend(path.segments().iter().map(AttachedPathSegment::source_text));
    parts.join(".")
}

fn canonical_part_modifier(source: &str, modifier: &AttachedViewPartModifier) -> Option<TextEdit> {
    if modifier.has_recovery() {
        return None;
    }
    let AttachedViewPartLocalName::Present(local_name) = modifier.local_name() else {
        return None;
    };
    let whole = modifier.whole().range();
    let replacement = format!(".part({})", source.get(local_name.range().as_range())?);
    let start = whole.start();
    let end = whole.end();
    (source.get(start..end)? != replacement).then_some(TextEdit {
        start,
        end,
        replacement,
    })
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
