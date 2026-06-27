use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_PRIVATE_FORMAT_MARKERS: &[&str] = &[
    "struct StringTable",
    "struct PublicIdTable",
    "struct StableIdTable",
    "struct ResourceStringTable",
    "struct ResourceReferenceTable",
    "struct DigestReferenceTable",
    "fn write_varint",
    "fn read_varint",
    "private string table",
    "private public id table",
];

#[test]
fn no_private_resource_wire_tables_outside_common_resource_codec() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = rust_files(&src).expect("bundle source tree is readable");
    let offenders = files
        .into_iter()
        .filter(|path| !is_common_resource_codec(path))
        .flat_map(|path| markers_in_file(&path))
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "new product resource section codecs must reuse arcweft_bundle::resource_codec shared tables/references instead of private wire formats:\n{}",
        offenders.join("\n")
    );
}

fn rust_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_rust_files(root, &mut out)?;
    Ok(out)
}

fn collect_rust_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn is_common_resource_codec(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.ends_with("src/resource_codec.rs") || normalized.contains("/src/resource_codec/")
}

fn markers_in_file(path: &Path) -> Vec<String> {
    let Ok(source) = fs::read_to_string(path) else {
        return vec![format!("{}: unreadable", path.display())];
    };
    FORBIDDEN_PRIVATE_FORMAT_MARKERS
        .iter()
        .filter(|marker| source.contains(**marker))
        .map(|marker| format!("{} contains `{marker}`", path.display()))
        .collect()
}
