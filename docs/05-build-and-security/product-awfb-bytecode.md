# Product AWFB bytecode after seq-01.5

New product `.awfb` files use `executable_payload = "awbc_v1"` in the product manifest and store `ProgramBytecode` as canonical AWBC bytes produced by `AwbcProgram::encode_canonical()`. Structured `BytecodeProgram` remains an inspection/export surface only and is not a product runtime payload. Old AWFB product bytecode wrappers that embed structured MessagePack or compact sidecars are rejected by product decoders with typed diagnostics.
