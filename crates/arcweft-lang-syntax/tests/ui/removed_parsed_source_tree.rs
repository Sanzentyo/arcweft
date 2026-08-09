use arcweft_lang_syntax::incremental::ParsedSource;

fn detached_whole_tree(parsed: &ParsedSource) {
    let _ = parsed.tree();
}

fn main() {}
