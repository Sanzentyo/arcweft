use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::parser::{AttachedFragment, ExpressionFragment};

fn lower_source_file(_: &ParsedSource) {}

fn reject_attached(fragment: &AttachedFragment<ExpressionFragment>) {
    lower_source_file(fragment);
}

fn main() {}
