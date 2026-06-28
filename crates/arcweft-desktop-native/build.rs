use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=native/macos/ArcweftTextInputClientView.swift");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let enabled = env::var_os("CARGO_FEATURE_MACOS_APPKIT_TEXT_INPUT").is_some();
    if target_os != "macos" || !enabled {
        return;
    }

    let source = Path::new("native/macos/ArcweftTextInputClientView.swift");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let bridge = out_dir.join("arcweft-macos-text-input-bridge");
    let swiftc = swiftc_path();

    let status = Command::new(&swiftc)
        .arg(source)
        .arg("-O")
        .arg("-whole-module-optimization")
        .arg("-framework")
        .arg("AppKit")
        .arg("-o")
        .arg(&bridge)
        .status()
        .unwrap_or_else(|error| panic!("failed to launch swiftc at {}: {error}", swiftc.display()));

    assert!(
        status.success(),
        "swiftc failed while building {}",
        bridge.display()
    );

    println!(
        "cargo:rustc-env=ARCWEFT_MACOS_TEXT_INPUT_BRIDGE={}",
        bridge.display()
    );
}

fn swiftc_path() -> PathBuf {
    let output = Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .expect("xcrun must be available on macOS when macos-appkit-text-input is enabled");
    assert!(
        output.status.success(),
        "xcrun --find swiftc failed; install Xcode command line tools"
    );
    let stdout = String::from_utf8(output.stdout).expect("xcrun output is utf-8");
    PathBuf::from(stdout.trim())
}
