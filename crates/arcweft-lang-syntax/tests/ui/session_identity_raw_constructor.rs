use arcweft_lang_syntax::incremental::SyntaxNodeId;
use core::num::NonZeroU64;

fn main() {
    let _forged = SyntaxNodeId(NonZeroU64::MIN);
}
