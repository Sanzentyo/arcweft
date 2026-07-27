use arcweft_lsp::{
    diagnostics::publish_diagnostics,
    features::character_definition::CharacterDefinitionSourceError,
    positions::LineIndex,
    profiles::LspProfileResolver,
    repl_command::LspReplCommandEndpoint,
};
use lsp_types::{Position, Uri};

fn removed_methods(
    index: &LineIndex,
    position: Position,
    resolver: &LspProfileResolver,
    uri: &Uri,
    endpoint: &mut LspReplCommandEndpoint<'_>,
) {
    let _ = index.byte_offset_from_position(position);
    let _ = resolver.resolve_for_uri(uri);
    let _ = endpoint.endpoint_mut();
}

fn main() {}
