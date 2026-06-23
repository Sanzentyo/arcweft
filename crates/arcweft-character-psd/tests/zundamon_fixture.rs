use arcweft_character::id::CharacterId;
use arcweft_character_psd::{PsdCharacterImportOptions, import_psd_character};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[test]
fn imports_optional_zundamon_v3_2_psd_fixture() {
    let Some(psd_path) = fixture_psd_path() else {
        eprintln!(
            "skipped optional PSD fixture test; set ARCWEFT_ZUNDAMON_PSD or copy the fixture into .arcweft-local/character-psd/zundamon-v3.2"
        );
        return;
    };

    let bytes = fs::read(&psd_path).expect("fixture PSD is readable");
    let character = CharacterId::try_new("character.zundamon").expect("character id");
    let options = PsdCharacterImportOptions::new(
        character,
        psd_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("zundamon.psd"),
    );
    let imported = import_psd_character(&bytes, &options).expect("zundamon PSD imports");
    let json = imported
        .manifest()
        .to_json_pretty()
        .expect("manifest serializes");

    assert!(imported.files().len() > 100);
    assert!(imported.manifest().parts().len() > 10);
    assert!(
        imported
            .warnings()
            .iter()
            .any(|warning| warning.contains("loose PSD group names"))
    );
    assert!(!json.contains(".arcweft-local"));
    assert!(!json.contains(&psd_path.display().to_string()));
    let source_file = imported
        .manifest()
        .source()
        .expect("source provenance")
        .file_name();
    assert!(
        Path::new(source_file)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("psd"))
    );
}

fn fixture_psd_path() -> Option<PathBuf> {
    env::var_os("ARCWEFT_ZUNDAMON_PSD")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| largest_psd_in_dir(&fixture_dir()?))
}

fn fixture_dir() -> Option<PathBuf> {
    env::var_os("ARCWEFT_ZUNDAMON_PSD_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            Some(
                manifest_dir
                    .ancestors()
                    .nth(2)?
                    .join(".arcweft-local")
                    .join("character-psd")
                    .join("zundamon-v3.2"),
            )
            .filter(|path| path.is_dir())
        })
}

fn largest_psd_in_dir(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("psd"))
        .filter_map(|path| {
            let len = path.metadata().ok()?.len();
            Some((len, path))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, path)| path)
}
