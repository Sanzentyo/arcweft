use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_verify_lsp::profile_hover;
use lsp_types::{Hover, Position};

/// Computes hover text for the word under the cursor.
pub fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<Hover> {
    let word = word_at_position(document, position)?;
    profile_hover(&profile.context(), &word)
}

pub(crate) fn word_at_position(document: &DocumentSnapshot, position: Position) -> Option<String> {
    let offset = document.line_index().byte_offset_from_position(position);
    let text = document.text();
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| text[start..end].to_owned())
}

fn is_symbol_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '@' | ':' | '-')
}
