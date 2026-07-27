use arcweft_lang_syntax::source::ParsedSource;

fn discard_document_owner(parsed: ParsedSource) {
    let _ = parsed.into_typed_tree();
}

fn main() {}
