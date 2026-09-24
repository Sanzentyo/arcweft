fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // The Windows main thread otherwise has a 1 MiB reserve. Checked game
    // entries use more stack than that during project compilation, while the
    // same source completes on the standard 2 MiB test-thread stack.
    let linker_arg = match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => "/STACK:2097152",
        Ok("gnu") => "-Wl,--stack,2097152",
        _ => return,
    };
    println!("cargo::rustc-link-arg-bin=arcw={linker_arg}");
}
