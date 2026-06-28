use std::fs;
use std::path::Path;

#[test]
fn web_player_does_not_install_hidden_text_entry_fallbacks() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "textarea",
        "contenteditable",
        "HtmlTextAreaElement",
        "HtmlInputElement",
        "set_attribute(\"contenteditable\"",
    ];

    let mut hits = Vec::new();
    for path in rust_files(&source_root) {
        let text = fs::read_to_string(&path).expect("web player source is utf-8");
        for pattern in forbidden {
            if text.contains(pattern) {
                hits.push(format!("{} contains `{pattern}`", path.display()));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "Web text input must use EditContext only, no hidden DOM fallback:\n{}",
        hits.join("\n")
    );
}

fn rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).expect("source directory exists") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}
