use arcweft_lang_hir::symbol::ExternalDeclarationSeed;

fn expose(seed: ExternalDeclarationSeed) {
    let ExternalDeclarationSeed {
        canonical_path,
        visibility,
        declaration,
        direct_bindings,
    } = seed;
    let _ = (canonical_path, visibility, declaration, direct_bindings);
}

fn main() {}
