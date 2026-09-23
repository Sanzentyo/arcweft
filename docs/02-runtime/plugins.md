# Rust / WASM plugin

## 実行形態

| 形態 | 用途 | 長所 | 短所 |
|---|---|---|---|
| static Rust Activity | 公式ゲーム、Web対応 | 最速、型安全 | hot reloadしにくい |
| native dylib | 開発中hot reload、信頼済みmod | 高速 | ABI管理が必要 |
| out-of-process | 重いActivity、クラッシュ隔離 | 安全 | IPC設計が重い |
| WASM component | mod、sandbox | 配布しやすい | zero-copy制約 |

## Rust export

Rust exports are opt-in adapter metadata, not source introspection. An
Arcweft-aware Rust crate annotates exported functions and ADTs with
`arcweft-rust-abi-macros`; its build writes deterministic
`arcweft-rust-abi` JSON into Cargo output or another project-relative metadata
location. `arcweft-rust-abi-build` is the build-script helper crate for writing
that JSON and emitting Cargo rerun hints, while `arcweft-rust-abi` remains data
and codecs only and the proc macros remain the source of truth for signatures.

Arcweft source does not redeclare the imported module shape. The schema-1
launch manifest admits one exact generated metadata artifact and a profile
selects the import that makes its mounted names visible to sema and LSP:

```toml
[external-modules.truck-game]
mount = "mini_games.truck"
metadata = "generated/truck-game.json"
metadata-hash = "blake3:1111111111111111111111111111111111111111111111111111111111111111"
expected-package = "org.example.truck-game"
expected-version = "1.0.0"
expected-module = "truck_game"
expected-family = "rust"
expected-abi-hash = "blake3:2222222222222222222222222222222222222222222222222222222222222222"
visibility = "package"
demand = "required"

[profiles.game]
kind = "game"
source = "src/main.arcw"
external-modules = ["truck-game"]
```

The loader verifies the artifact digest and declared identity before projecting
its typed public exports. A direct source check or a profile that omits the
import receives no generated bindings; there is no dynamic fallback or
source-authored alias layer.

Adapter metadata is carried by `arcweft-adapter-context` as an
`AdapterManifest`. Standard manifests such as `sans-io`, `native-http`,
`inference-tensor`, `system-info`, `native-file`, and `math` are resolved
through the standard adapter registry. A manifest can contribute source-visible
symbols and methods, typed free functions, granted or required effect
capabilities, host-call identifiers, tooling docs, and merged Rust ABI exports.
This keeps core language parsing independent from adapter-specific names while
still giving CLI, verifier, and LSP one typed source of truth.

`arcweft-adapter-context` remains a language-free data boundary so runtime
hosts can consume the same selected manifest without acquiring syntax, HIR, or
semantic-analysis dependencies. `arcweft-adapter-sema` is the compiler-side
bridge that turns an admitted `AdapterManifest` into source-backed external,
nominal, callable, and effect facts. Runtime crates do not depend on that
projection crate.

Standard adapters use the typed [Adapter Manifest
Schema](../schemas/adapter-manifest.md). Effect labels are stored as
`EffectCapability` ids with parsed family/operation/scope components, while
host calls use stable ids such as `fs.read_text` or `custom.read`. Generated
external-module metadata is projected into the same typed semantic view only
after its schema-1 import has been admitted; a profile does not load a second
project-local adapter-manifest reader.

Minimal adapter build scripts construct the manifest explicitly and delegate
file I/O to `arcweft-rust-abi-build`:

```rust
use arcweft_rust_abi::{
    ArcweftRustCallableRole, ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage,
    ArcweftRustPackageId, ArcweftRustTypePath,
    ArcweftRustParam, ArcweftRustPurity, ArcweftRustTypeRef,
};
use arcweft_rust_abi_build::{
    MetadataBuildOptions, emit_cargo_rerun_hints, write_manifest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = ArcweftRustManifest::builder(ArcweftRustPackage {
        id: ArcweftRustPackageId::try_new("truck_game")?,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        metadata_hash: None,
    })
    .with_function(ArcweftRustFunction {
        name: "mini_games.truck.score_to_rank".to_owned(),
        rust_path: "truck_game::score_to_rank".to_owned(),
        role: ArcweftRustCallableRole::Function,
        params: vec![ArcweftRustParam {
            name: "score".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::Nominal {
            package: ArcweftRustPackageId::try_new("truck_game")?,
            path: ArcweftRustTypePath::try_new(["Rank".try_into()?])?,
            arguments: Vec::new(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
    .build();
    let options = MetadataBuildOptions::from_out_dir_env("truck_game")?
        .with_rerun_if_changed("src/lib.rs");
    emit_cargo_rerun_hints(&options);
    write_manifest(&manifest, &options)?;
    Ok(())
}
```

Non-Arcweft-aware Rust crates are exposed through a small annotated wrapper
crate. Raw pointers, unsafe ABIs, non-static borrows, and unsupported generic
exports are rejected by the metadata macro rather than accepted as dynamic
fallbacks.

### Rust data policy and field defaults

`ArcweftType` and `ArcweftReflect` use the same `#[arcweft(...)]` attribute
grammar. The ABI declaration retains resolved field and case wire names,
record unknown-field policy, enum tag/content/repr/discriminants, field
Bytes format, and explicit default/skip intent. Repr discriminants use
canonical decimal strings in schema-1 JSON so the full signed 128-bit domain
round-trips without JSON numeric narrowing. Accepted metadata carries these
properties into the nominal source graph; its canonical layout operation
commits them. Per-occurrence policies remain attached to the selected program
rows, so a shared logical Bytes type can have different field wire formats.

Rust `Encode`/`Decode` derives preserve `Value::Enum` for every enum tagging
and repr policy. Codecs apply those policies using the selected `TypeShape`;
the typed value is not preformatted into a record or numeric discriminant.

Metadata collection never executes a field default. A path default such as
`#[arcweft(default = "default_flag")]` names an `#[arcweft_export(pure)]`
function. The generated metadata joins that function's actual registered
declaration and checks its nullary signature and exact field result type.
Trait defaults and skipped fields require an explicitly exported pure
constructor for the concrete field type:

```rust
#[derive(arcweft_rust_abi_macros::ArcweftType)]
pub struct Cache(bool);

#[arcweft_rust_abi_macros::arcweft_export(pure, name = "default_cache")]
impl Default for Cache {
    fn default() -> Self { Self(false) }
}
```

The manifest must register the emitted `default_cache` callable metadata.
This is an explicit purity contract for the real generated wrapper; merely
implementing Rust `Default` does not grant it. Accepted sema verifies a unique
registered callable, empty effect row, no inputs, exact result semantic type,
and matching checked publication generation. The program ID commits the
complete declaration/signature proof, not the source path alone. The selected
Plan/AWBC retains the existing pure helper and binding, and each default field
stores its exact `default_program` edge. Decode requests that field through a
Sans-I/O provider; the provider executes the selected pure program through the
host's admitted backend and validates its returned runtime value against the
field row before conversion. A caller without a provider receives an explicit
missing-default error. `has_default` alone never synthesizes a value.

For an existing primitive or foreign `Default` implementation, declare the
same concrete wrapper without writing an orphan trait implementation:

```rust
arcweft_rust_abi_macros::arcweft_export_default!(pure, pub fn default_flag() -> bool);
arcweft_rust_abi_macros::arcweft_export_default!(pure, pub fn default_text() -> String);
```

The generated body calls the exact `core::default::Default` implementation.
These declarations require no arguments and a concrete result type. A generic
record field binds to the corresponding concrete wrapper after instantiation;
missing, wrong-result, or duplicate constructor declarations fail admission.

## WASM plugin

WASM is a plugin/activity sandbox format, not Arcweft's primary script runtime.
Arcweft scripts lower to Typed IR / bytecode and run on the VM. A native player may
use `arcweft-wasm-wasmtime` for sandboxed plugin calls; a browser player uses its
own Wasm player build and browser APIs, not Wasmtime.

The plugin ABI is described with WIT in `arcweft-wasm-abi`. Validation,
component generation, and inspection live in `arcweft-wasm-tools`; host execution
adapters stay outside `arcweft-core`.

```rust
wasm plugin affection_ai from "plugins/affection_ai.wasm" {
    abi = "wit:arcweft:plugin/affection@0.1.0"
    sandbox {
        memory = 8MiB
        fuel_per_call = 2_000_000
        wasi = false
        network = false
        filesystem = false
    }
    import fn score(state: GameState, event: ChoiceEvent) -> i32
}
```

## Security

- filesystem/network は deny by default。
- host import whitelist。
- fuel / memory / call time limit。
- WASI preopen なしがデフォルト。
- Activity から直接 engine state を mutate しない。

