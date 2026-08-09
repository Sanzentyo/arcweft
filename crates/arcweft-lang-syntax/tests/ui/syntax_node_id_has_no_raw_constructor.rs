use arcweft_lang_syntax::attachment::{SyntaxLineageId, SyntaxNodeId};
use core::num::NonZeroU64;

fn forge(lineage: SyntaxLineageId, slot: NonZeroU64) -> SyntaxNodeId {
    SyntaxNodeId { lineage, slot }
}

fn main() {}
