use arcweft_character::{
    id::{CharacterId, CharacterLookId},
    manifest::CharacterPoint,
};
use arcweft_character_psd::{PsdCharacterImportOptions, import_psd_character};
use clap::{Args, Subcommand};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const MANIFEST_FILE_NAME: &str = "character.awchar.json";

#[derive(Clone, Debug, Subcommand)]
pub(in crate::app) enum ImportCommand {
    /// Convert a layered Photoshop PSD into an Arcweft character package.
    PsdCharacter(PsdCharacterOptions),
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct PsdCharacterOptions {
    /// Source PSD file.
    input: PathBuf,
    /// Public Arcweft character id, for example `character.akane`.
    #[arg(long)]
    character: String,
    /// Destination `.awchar` package directory.
    #[arg(short, long)]
    output: PathBuf,
    /// Look to use when the character is shown without an explicit look.
    #[arg(long)]
    default_look: Option<String>,
    /// Character anchor x coordinate in PSD canvas pixels.
    #[arg(long, requires = "anchor_y")]
    anchor_x: Option<i32>,
    /// Character anchor y coordinate in PSD canvas pixels.
    #[arg(long, requires = "anchor_x")]
    anchor_y: Option<i32>,
    /// Treat importer compatibility warnings as errors.
    #[arg(long)]
    strict: bool,
    /// Replace an existing destination package.
    #[arg(long)]
    force: bool,
    /// Emit a machine-readable conversion report.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Serialize)]
struct PsdCharacterReport {
    character: String,
    output: String,
    manifest: String,
    files: usize,
    warnings: Vec<String>,
}

pub(super) fn import_command(command: ImportCommand) -> Result<(), ExitCode> {
    match command {
        ImportCommand::PsdCharacter(options) => import_psd_character_command(&options),
    }
}

fn import_psd_character_command(options: &PsdCharacterOptions) -> Result<(), ExitCode> {
    let bytes = fs::read(&options.input).map_err(|error| {
        eprintln!(
            "error: failed to read PSD {}: {error}",
            options.input.display()
        );
        ExitCode::FAILURE
    })?;
    let character = CharacterId::try_new(&options.character).map_err(|error| {
        eprintln!("error: invalid --character: {error}");
        ExitCode::from(2)
    })?;
    let mut import_options =
        PsdCharacterImportOptions::new(character.clone(), options.input.display().to_string())
            .strict(options.strict);
    if let Some(default_look) = options.default_look.as_deref() {
        let default_look = CharacterLookId::try_new(default_look).map_err(|error| {
            eprintln!("error: invalid --default-look: {error}");
            ExitCode::from(2)
        })?;
        import_options = import_options.with_default_look(default_look);
    }
    if let (Some(x), Some(y)) = (options.anchor_x, options.anchor_y) {
        import_options = import_options.with_anchor(CharacterPoint::new(x, y));
    }
    let imported = import_psd_character(&bytes, &import_options).map_err(|error| {
        eprintln!("error: PSD character import failed: {error}");
        ExitCode::FAILURE
    })?;
    write_character_package(&options.output, &imported, options.force).map_err(|error| {
        eprintln!(
            "error: failed to write character package {}: {error}",
            options.output.display()
        );
        ExitCode::FAILURE
    })?;

    let report = PsdCharacterReport {
        character: character.to_string(),
        output: options.output.display().to_string(),
        manifest: options
            .output
            .join(MANIFEST_FILE_NAME)
            .display()
            .to_string(),
        files: imported.files().len(),
        warnings: imported.warnings().to_vec(),
    };
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializable import report")
        );
    } else {
        println!(
            "imported {} to {} ({} image file(s))",
            report.character, report.output, report.files
        );
        for warning in &report.warnings {
            eprintln!("warning: {warning}");
        }
    }
    Ok(())
}

fn write_character_package(
    output: &Path,
    imported: &arcweft_character_psd::ImportedCharacter,
    force: bool,
) -> Result<(), std::io::Error> {
    if output.exists() && !force {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "destination exists; pass --force to replace it",
        ));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("character.awchar");
    let staging = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }

    let result = (|| {
        fs::create_dir(&staging)?;
        for file in imported.files() {
            let destination = staging.join(file.path().as_str());
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, file.bytes())?;
        }
        let manifest = imported
            .manifest()
            .to_json_pretty()
            .map_err(std::io::Error::other)?;
        fs::write(staging.join(MANIFEST_FILE_NAME), manifest)?;
        if output.exists() {
            if output.is_dir() {
                fs::remove_dir_all(output)?;
            } else {
                fs::remove_file(output)?;
            }
        }
        fs::rename(&staging, output)
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn clap_accepts_the_psd_character_surface() {
        use clap::Parser;

        let cli = super::super::commands::Cli::try_parse_from([
            "arcw",
            "import",
            "psd-character",
            "art/akane.psd",
            "--character",
            "character.akane",
            "--output",
            "assets/akane.awchar",
        ]);
        assert!(cli.is_ok());
    }
}
