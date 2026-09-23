use arcweft_adapter_context::manifest::{AdapterManifest, AdapterNominalPathPrefix};
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId,
    ArcweftRustPurity, ArcweftRustTypeRef,
};

fn manifest(package: &str, names: &[&str]) -> ArcweftRustManifest {
    names.iter().enumerate().fold(
        ArcweftRustManifest::new(ArcweftRustPackage {
            id: ArcweftRustPackageId::try_new(package).unwrap(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        }),
        |manifest, (index, name)| {
            manifest.with_function(ArcweftRustFunction {
                role: Default::default(),
                name: (*name).to_owned(),
                rust_path: format!("{package}::function_{index}"),
                params: vec![],
                return_type: ArcweftRustTypeRef::Unit,
                purity: ArcweftRustPurity::Pure,
                effects: vec![],
            })
        },
    )
}

#[test]
fn rust_overloads_are_contiguous_per_callable_path_across_package_publications() {
    let first = manifest("first_package", &["alpha", "beta", "alpha"]);
    let second = manifest("second_package", &["beta", "gamma", "alpha"]);
    let adapter = AdapterManifest::new("fixture", "Overload fixture")
        .try_with_rust_package_mount(
            first.package.id.clone(),
            AdapterNominalPathPrefix::try_new([]).unwrap(),
        )
        .unwrap()
        .try_with_rust_package_mount(
            second.package.id.clone(),
            AdapterNominalPathPrefix::try_new([]).unwrap(),
        )
        .unwrap()
        .try_with_rust_manifest(&first)
        .unwrap()
        .try_with_rust_manifest(&second)
        .unwrap();
    assert_eq!(
        adapter
            .rust_functions()
            .iter()
            .map(|function| function.overload().get())
            .collect::<Vec<_>>(),
        [0, 0, 1, 1, 0, 2]
    );
}
