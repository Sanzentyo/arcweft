use arcweft_lang_sema::nominal::NominalResolutionIndex;
use arcweft_lang_syntax::types::TypeRefNodePath;
use arcweft_source::SourceSpan;

fn removed_helper(
    index: &NominalResolutionIndex,
    root: &SourceSpan,
    node: &TypeRefNodePath,
) {
    let _ = index.recovered_node_type(root, node);
}

fn main() {}
