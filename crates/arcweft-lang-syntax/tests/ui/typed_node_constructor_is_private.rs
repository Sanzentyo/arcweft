use arcweft_lang_syntax::attachment::{AstNode, SourceFileKind, SyntaxNodeHandle};

fn forge(handle: SyntaxNodeHandle) -> AstNode<SourceFileKind> {
    AstNode::<SourceFileKind>::new(handle).unwrap()
}

fn main() {}
