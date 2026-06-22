use arcweft_bundle::ArcweftBundle;
use clap::Args;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct NativePlayerOptions {
    bundle: PathBuf,
    #[arg(long, default_value_t = 64)]
    steps: usize,
}

pub(super) fn native_player_command(options: &NativePlayerOptions) -> Result<(), ExitCode> {
    let bytes = fs::read(&options.bundle).map_err(|error| {
        eprintln!(
            "error: could not read bundle {}: {error}",
            options.bundle.display()
        );
        ExitCode::FAILURE
    })?;
    let bundle = ArcweftBundle::from_path_slice(&options.bundle, &bytes).map_err(|error| {
        eprintln!(
            "error: could not decode bundle {}: {error}",
            options.bundle.display()
        );
        ExitCode::FAILURE
    })?;
    arcweft_player_native::run_bundle_windowed(bundle, options.steps).map_err(|error| {
        eprintln!("error: native player failed: {error}");
        ExitCode::FAILURE
    })
}
