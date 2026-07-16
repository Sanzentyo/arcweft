//! Private Flow body grammar over the shared full-source cursor.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::lexer::LexToken;
use super::shadow_recovery::{bump_until, find_top_level_boundary};
use super::statement::emit_braced_block;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events);
    parser.start(SyntaxKind::FlowItem, role);
    let open = find_top_level_boundary(&parser, parser.cursor(), &["{"]);
    bump_until(&mut parser, open);

    parser.start(SyntaxKind::FlowBody, SyntaxRole::Body);
    if parser.at("{") {
        emit_braced_block(
            &mut parser,
            SyntaxKind::FlowItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.flow.missing_block_close",
        );
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.flow.missing_body",
            SourceRange::new(at, at),
            "missing Flow body",
        )));
    }
    parser.finish();

    while parser.bump().is_some() {}
    parser.finish();
}
