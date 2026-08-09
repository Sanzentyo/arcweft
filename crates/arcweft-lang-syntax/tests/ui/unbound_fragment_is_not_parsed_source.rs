use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::parser::{ExpressionFragment, UnboundFragment};

fn lower_source_file(_: &ParsedSource) {}

fn reject_unbound(fragment: &UnboundFragment<ExpressionFragment>) {
    lower_source_file(fragment);
}

fn main() {}
