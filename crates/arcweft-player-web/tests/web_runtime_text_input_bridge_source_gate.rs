use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn normal_web_player_has_no_sample_owned_text_input_session() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.join("../..");
    let mut sources = rust_files(&manifest.join("src"));
    sources.extend([
        repo_root.join("web/player.js"),
        repo_root.join("web/player-editcontext.js"),
        repo_root.join("web/ime-sample.js"),
    ]);
    let forbidden = [
        "sample_snapshot",
        "WEB_TEXT_INPUT_SESSIONS",
        "PlayerOwnedTextInputSession",
        "arcweft_web_text_input_activate(",
        "arcweft_web_text_input_deactivate(",
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
        "normal Web player text input must be runtime-owned, not sample-owned:\n{}",
        hits.join("\n")
    );
}

#[test]
fn hidden_dom_editing_fallbacks_remain_absent() {
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
