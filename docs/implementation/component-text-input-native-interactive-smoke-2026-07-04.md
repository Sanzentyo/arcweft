# Component text input native interactive smoke

Date: 2026-07-04
Sequence: seq06.16.3

## Status

This cut turns the native text-input window smoke into an explicit, repeatable
manual evidence fixture. The authored language/resource contract is unchanged:
component/View `TextField`, `TextArea`, and `SecureField` remain the source of
input resources, and no top-level `ui text_input`, `ui text_area`, or
`ui secure_field` declarations are reintroduced.

The smoke remains manual because a real native window, real keyboard focus, and
a platform IME are environment-dependent. The repository implementation added by
this cut provides:

- a focused integration test that guards the selected samples and command
  contract;
- a refreshed seq06.4j.1 source gate that now matches the component-authored,
  no-sidecar input contract;
- a trace verifier for the manual run output;
- Justfile entrypoints that separate preflight/bundle checks from the blocking
  native interactive window launch.

## Arcweft boundary fit

The smoke stays inside the normal native player route. It does not add platform
widgets, DOM controls, hidden native controls, or a second text-input backend.
The expected route is:

```text
component/View text control
  -> component lowering / UiInputResource
  -> player-rendered runtime text control
  -> native player winit text-input bridge
  -> shared player text editor / text_submit write-back
```

## Selected samples

| Sample | Role in this smoke | Required outcome |
| --- | --- | --- |
| `samples/native-text-input` | Primary native window/IME sample. It has `TextField`, multiline `TextArea`, and secure `SecureField`. | Launch through `arcw run --runner native`; collect a trace under `target/native-text-input-trace/seq06.16.3/`. |
| `samples/text-submit-flow` | Submit parity sample. | Source and bundle checks prove Enter/IME submit and Button `.on_click` use the same `text_submit` target. |
| `samples/modern-feedback-ui` | Component/View integration sample. | Source and bundle checks prove component-authored text controls and submit buttons remain available. This is not a style-resolution acceptance for seq06.16.4. |

## Commands

Preflight and bundle checks:

```bash
just component-text-input-native-smoke-check
```

Equivalent expanded commands:

```bash
cargo test -p arcweft-cli --test native_text_input_sample_sidecars --quiet
cargo test -p arcweft-cli --test native_text_input_native_interactive_smoke --quiet
cargo run -p arcweft-cli -- check --manifest-path samples/native-text-input/arcw.toml
cargo run -p arcweft-cli -- check --manifest-path samples/text-submit-flow/arcw.toml
cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-ui/arcw.toml
cargo run -p arcweft-cli -- bundle samples/native-text-input/src/main.arcw --output target/arcweft/native-text-input-seq06.16.3.awfb
cargo run -p arcweft-cli -- bundle samples/text-submit-flow/src/main.arcw --output target/arcweft/text-submit-flow-seq06.16.3.awfb
cargo run -p arcweft-cli -- bundle samples/modern-feedback-ui/src/main.arcw --output target/arcweft/modern-feedback-ui-seq06.16.3.awfb
```

Manual native window launch:

```bash
cargo run -p arcweft-cli --features native-player -- run   --runner native samples/native-text-input/src/main.arcw   --text-input-trace-out target/native-text-input-trace/seq06.16.3/native-player-ime.real.json
```

Optional ignored test wrapper:

```bash
ARCWEFT_SEQ06_16_3_INTERACTIVE=1 cargo test -p arcweft-cli --features native-player   --test native_text_input_native_interactive_smoke   seq06_16_3_launch_native_player_for_manual_smoke   -- --ignored --exact --nocapture
```

Trace gate after closing the window:

```bash
cargo +nightly -Zscript tools/verify-seq06-16-3-native-smoke-trace.rs   --trace target/native-text-input-trace/seq06.16.3/native-player-ime.real.json
```

## Manual interaction checklist

Record the operator, OS, compositor/window system, backend, GPU adapter, DPI,
keyboard layout, active IME, and exact command line before starting.

| Step | Action | Required evidence |
| --- | --- | --- |
| 1 | Launch `samples/native-text-input` with `--runner native` and `--text-input-trace-out`. | Command log shows the bundle build and native player start. Window remains interactive until closed. |
| 2 | Pointer-focus `jp_text_field`. | Trace has `focus` and `geometry` records for `jp_text_field`. |
| 3 | Type Latin text and Japanese IME text in `jp_text_field`. | Trace has `routed_text_input` and `runtime_write_back` `change` records. |
| 4 | Press Enter or the IME done/send action in `jp_text_field`. | Trace has a `runtime_write_back` `submit` record for the same input target. |
| 5 | Keyboard-traverse to `jp_text_area`. | Focus generation changes and target becomes `jp_text_area`. |
| 6 | Type at least two lines in `jp_text_area` using Enter/newline where supported. | Multiline editing is visible; trace mentions `jp_text_area` and routed input records. |
| 7 | Focus `secret_secure_field` and enter text. | SecureField renders masked text; trace has `secure_redacted=true`, `value_len=0` for secure write-back, and does not contain `sekret-1234`. |
| 8 | Activate submit by keyboard and by Button route in `samples/text-submit-flow`. | Flow reaches the same `text_submit` target as the Button `.on_click(ime: .commit)` source route. |
| 9 | Exercise close/cancel behavior. | Native player exits cleanly after the window is closed or cancel is requested. |

## Trace and observation requirements

Save artifacts under `target/native-text-input-trace/seq06.16.3/`:

- `native-player-ime.real.json`: native text-input trace from the launch command.
- `command.log`: full terminal output for preflight, launch, and trace gate.
- `environment.json`: OS/backend/adapter/DPI/keyboard/IME fingerprint.
- Optional screenshots or Agent observe JSON if available. Screenshots and
  observe JSON are supportive only; the acceptance route is the player text-input
  trace plus observed window interaction.

SecureField does not expose `sekret-1234` in the trace or observation output.
The trace gate fails if that probe value appears anywhere in the trace or any
provided observation JSON.

## Implemented files

- `crates/arcweft-cli/tests/native_text_input_native_interactive_smoke.rs`
- `tools/verify-seq06-16-3-native-smoke-trace.rs`
- `tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs`
- `Justfile` recipes: `component-text-input-native-smoke-check` and
  `component-text-input-native-smoke`
- this implementation note

## Validation status for this package

Validated from source inspection against the latest available `main` snapshot via
the GitHub connector:

- `AGENTS.md`, `docs/README.md`, and `docs/00-overview/architecture.md` were read
  before designing the change.
- `docs/implementation/component-text-input-unification-2026-07-04.md` already
  records that the three selected samples bundle without top-level text-control
  declarations.
- `samples/native-text-input/src/main.arcw`, `samples/text-submit-flow/src/main.arcw`,
  and `samples/modern-feedback-ui/src/main.arcw` were inspected and are the basis
  for the sample selection above.

Packaging-environment blockers:

- direct `git clone https://github.com/Sanzentyo/arcweft.git` failed because the
  sandbox could not resolve `github.com`;
- `rustc` and `cargo` are not installed in the packaging sandbox;
- no real native desktop/IME run was executed in the packaging sandbox.

Therefore this package does not claim a real-machine smoke pass. It provides the
implementation and evidence harness that must be run on a native machine, and it
keeps the pass/fail fields explicit for the operator's resulting evidence packet.

## Receiving checkout validation

Applied in `D:\git\arcweft` on Jujutsu change
`mqstkkvzwsrwrrpuqlvymukwzuxuovqr`.

The bundled patch file failed `git apply --check` with `corrupt patch at line
236`, so the checked-in overlay files were applied directly and the Justfile
recipes were inserted manually.

Commands run after application:

```bash
cargo fmt --all
cargo test -p arcweft-cli --test native_text_input_sample_sidecars --quiet
cargo test -p arcweft-cli --test native_text_input_native_interactive_smoke --quiet
cargo +nightly -Zscript tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs --root .
just component-text-input-native-smoke-check
cargo +nightly -Zscript tools/verify-seq06-16-3-native-smoke-trace.rs --help
cargo fmt --all -- --check
cargo clippy -p arcweft-cli --all-targets -- -D warnings
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-cli --all-targets -- -D warnings
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structure audit scanned 2,326 files and 1,117 Rust files, reporting the
current workspace baseline of `4 error(s), 127 warning(s)` as a dry run with no
report files written.

Changed Rust file measurements:

| Path | Owner | Kind | Bytes | Physical LOC | Embedded test LOC | Responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-cli/tests/native_text_input_native_interactive_smoke.rs` | `arcweft-cli` | integration test | 5,877 | 172 | 0 | Sample contract, shared submit-route guard, documentation/Justfile/trace-gate guard, ignored real-window launch wrapper. |
| `tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs` | workspace tool | Cargo script source gate | 4,106 | 136 | 0 | Component-authored native text-control source gate, removed sidecar guard, native bridge/source-route checks. |
| `tools/verify-seq06-16-3-native-smoke-trace.rs` | workspace tool | Cargo script verifier | 6,521 | 210 | 0 | Native text-input trace JSON validation, secure redaction checks, optional observation leak checks. |

Largest workspace Rust hotspots observed during the audit sample, unchanged by
this cut:

| Path | Bytes | Physical LOC | Kind |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | production |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,414 | 7,944 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243,051 | 6,758 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222,475 | 6,161 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209,852 | 5,651 | integration test |

## Remaining gaps

- Real Windows/macOS/Linux IME acceptance must be captured on machines with the
  target OS, display server, keyboard layout, and IME.
- Mobile and Web validation are out of scope for this native-only smoke.
- Modern feedback UI style-to-runtime-control rendering remains tracked by
  seq06.16.4; this smoke only checks that the component-authored text-control and
  submit routes remain launchable through the player-backed path.
