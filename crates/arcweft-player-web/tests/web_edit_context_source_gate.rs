use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn web_player_does_not_install_hidden_text_entry_fallbacks() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.join("../..");
    let mut sources = rust_files(&manifest.join("src"));
    sources.extend([
        repo_root.join("web/player.js"),
        repo_root.join("web/player-editcontext.js"),
        repo_root.join("web/ime-sample.js"),
    ]);
    let forbidden = [
        "textarea",
        "contenteditable",
        "HtmlTextAreaElement",
        "HtmlInputElement",
        "set_attribute(\"contenteditable\"",
        "installKeyboardFallback",
        "beforeinput",
    ];

    let hits = sources
        .into_iter()
        .filter(|path| path.exists())
        .flat_map(|path| {
            let text = fs::read_to_string(&path).expect("web player source is utf-8");
            forbidden
                .iter()
                .filter(move |pattern| text.contains(**pattern))
                .map(move |pattern| format!("{} contains `{pattern}`", path.display()))
        })
        .collect::<Vec<_>>();

    assert!(
        hits.is_empty(),
        "Web text input must use EditContext only, no hidden DOM fallback:\n{}",
        hits.join("\n")
    );
}

#[test]
fn ime_sample_does_not_own_editcontext_events_or_model_state() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("../../web/ime-sample.js");
    let text = fs::read_to_string(&sample).expect("sample source is utf-8");
    let forbidden = [
        "new window.EditContext",
        "new EditContext",
        "addEventListener(\"textupdate\"",
        "addEventListener('textupdate'",
        "addEventListener(\"compositionend\"",
        "addEventListener('compositionend'",
        "let modelText",
        "modelText =",
        "applyUpdate(",
    ];

    let hits = forbidden
        .iter()
        .filter(|pattern| text.contains(**pattern))
        .map(|pattern| format!("{} contains `{pattern}`", sample.display()))
        .collect::<Vec<_>>();

    assert!(
        hits.is_empty(),
        "IME sample must be a thin consumer of player-owned glue:\n{}",
        hits.join("\n")
    );
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .expect("source directory exists")
        .flat_map(|entry| {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                rust_files(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}
