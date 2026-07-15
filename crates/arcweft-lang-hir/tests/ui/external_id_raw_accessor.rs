use arcweft_lang_hir::symbol::ExternalDeclarationId;

fn raw(id: ExternalDeclarationId) -> u32 {
    id.index()
}

fn main() {}
