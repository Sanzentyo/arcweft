# Project Helper Wrapper Review Follow-up

Date: 2026-06-22

Source package:
`D:/sanze/Downloads/arcweft-project-rs-wrapper-review-2026-06-21.zip`

## Applied Findings

- `F-03`: `arcw serve --manifest ... --profile ...` now passes the actual CLI
  adapter override state into profile checking and host-policy resolution. A
  profile-selected adapter no longer masquerades as a command-line override, so
  profile `rust_metadata` is merged during serve planning.
- `F-04`: the serve adapter display fallback now uses
  `arcweft_adapter_context::standard::SANS_IO_ADAPTER_ID` instead of an inline
  `"sans-io"` literal.
- `F-01`: the single-use `adapter_manifest_from_registry` wrapper was removed;
  default adapter resolution and registry lookup now live directly in
  `adapter_manifest_for_selection`.
- `F-02`: launch-profile listen parsing moved out of the shared project helper
  surface and into the serve command path that owns `SocketAddr` diagnostics.
- `F-05`: project-local adapter manifest and Rust ABI metadata filesystem
  loading moved into `arcweft-project-loader`. The new crate owns read,
  format dispatch, and decode only. CLI keeps fail-fast stderr policy, while
  LSP keeps diagnostic accumulation and profile-relative resource labels.

## Verification Coverage

- Added a serve regression where a server launch profile selects
  `native-http`, lists `rust_metadata`, and typechecks an `extern rust mod`
  call. This reproduces the old override-origin bug because the previous serve
  path skipped profile Rust metadata after resolving the profile adapter into
  `Some(adapter)`.
- Updated CLI documentation to state that dedicated profile commands preserve
  profile Rust metadata when the adapter comes from the profile, while an
  explicit `--adapter` remains an override.

## Validation Run

- `cargo test -p arcweft-project-loader`: passed.
- `cargo test -p arcweft-lsp`: passed.
- `cargo test -p arcweft-cli --test check
  serve_profile_preserves_rust_metadata_when_adapter_comes_from_profile --
  --exact --nocapture`: passed.
- `cargo test -p arcweft-cli --test check profile_check_reports_adapter_manifest -- --nocapture`:
  passed.
- `cargo test -p arcweft-cli --test check profile_check_reports_rust_metadata -- --nocapture`:
  passed.
- `cargo check -p arcweft-project-loader -p arcweft-lsp -p arcweft-cli --all-features`:
  passed.
- `cargo clippy -p arcweft-project-loader -p arcweft-lsp -p arcweft-cli
  --all-targets --all-features -- -D warnings`: passed.
- `cargo fmt --all --check`: passed.
- `cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .`:
  passed with `0 error(s), 87 warning(s)`.
- `just test-cli-check`: passed.
- `just test-workspace`: passed after regenerating `web/demo.awfb` to the
  current bundle schema.

## Follow-up Boundaries

- `F-05` is complete as a small host/tooling I/O boundary. Do not reintroduce
  local adapter-manifest or Rust-metadata filesystem loaders in CLI or LSP, and
  do not move this path I/O into Sans I/O data crates.
- The reviewed launch backend mapping helpers remain in place. A direct
  `From<LaunchPureBackend>` / `From<LaunchMathBackend>` implementation in
  `arcweft-cli` would violate Rust's orphan rules, and moving the conversion
  into either owned crate would introduce a larger layer dependency decision.
- `native_host_policy_for_selection` remains in place because it protects the
  default path from spreading `None`/override semantics across ordinary
  callers.
