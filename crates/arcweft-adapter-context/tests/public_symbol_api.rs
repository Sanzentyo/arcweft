use arcweft_adapter_context::manifest::{
    AdapterManifest, AdapterSymbol, AdapterSymbolPath, AdapterSymbolSegment, AdapterTypeKind,
};

fn symbol_path() -> AdapterSymbolPath {
    AdapterSymbolPath::try_new([
        AdapterSymbolSegment::try_new("adapter").expect("namespace segment"),
        AdapterSymbolSegment::try_new("hero-pack").expect("external segment"),
    ])
    .expect("public typed adapter path")
}

#[test]
fn public_typed_symbol_api_compiles_without_sema() {
    let path = symbol_path();
    let manifest = AdapterManifest::new("public-api", "Public API")
        .with_symbol(AdapterSymbol::new(path.clone(), AdapterTypeKind::I32));

    assert_eq!(manifest.symbols()[0].path(), &path);
}
