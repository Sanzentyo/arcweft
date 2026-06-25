# Product AWFB bytecode after seq-01.5

New product `.awfb` files use `executable_payload = "awbc_v1"` in the product manifest and store `ProgramBytecode` as canonical AWBC bytes produced by `AwbcProgram::encode_canonical()`. Structured `BytecodeProgram` remains an inspection/export surface only and is not a product runtime payload. Old AWFB product bytecode wrappers that embed structured MessagePack or compact sidecars are rejected by product decoders with typed diagnostics.


## Producer contract after seq-01.6

Normal source, project, profile, watch, native-run, and web-run product builds
lower canonical AWBC in the compiler-side runtime-plan lowering layer before
bundle encoding. The shared builder attaches the resulting `AwbcProgram`
through `ArcweftBundle::with_product_awbc`; the bundle codec does not depend on
compiler, filesystem, host, signing, clock, or platform crates.

AWBC lowering diagnostics are reported at the product build phase. The codec's
missing-AWBC diagnostic remains a defensive invariant for manually constructed
bundles, not the normal producer error path. Source/dev execution may continue
to use structured bytecode, but an `.awfb` Game product has no structured
executable fallback.
