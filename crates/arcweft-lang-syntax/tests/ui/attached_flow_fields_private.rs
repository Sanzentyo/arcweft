use arcweft_lang_syntax::attachment::node::{ErrorNodeKind, FlowItemKind};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedFlowContractClause, AttachedFlowDeclaration, AttachedFlowIdentity,
    AttachedFlowSignature, AttachedItemPrefix, AttachedRequiredFlowBody,
};
use arcweft_source::SourceSpan;

fn forge(
    syntax: AstNode<FlowItemKind>,
    prefix: AttachedItemPrefix,
    keyword: SourceSpan,
    identity: AttachedFlowIdentity,
    signature: AttachedFlowSignature,
    contracts: Box<[AttachedFlowContractClause]>,
    body: AttachedRequiredFlowBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
) -> AttachedFlowDeclaration {
    AttachedFlowDeclaration {
        syntax,
        prefix,
        keyword,
        identity,
        signature,
        contracts,
        body,
        trailing_recovery,
    }
}

fn main() {}
