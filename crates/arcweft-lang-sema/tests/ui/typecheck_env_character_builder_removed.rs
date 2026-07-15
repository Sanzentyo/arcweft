use arcweft_character::manifest::CharacterManifest;
use arcweft_lang_sema::env::TypeCheckEnv;

fn bypass(environment: TypeCheckEnv, manifest: CharacterManifest) {
    let _ = environment.with_character_manifest(manifest);
}

fn main() {}
