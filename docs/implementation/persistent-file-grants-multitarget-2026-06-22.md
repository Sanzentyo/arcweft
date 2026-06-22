# Persistent File Grants Multitarget, 2026-06-22

This note records the implementation taken from
`arcweft-persistent-file-grants-multitarget-2026-06-22.zip`.

## Package Condition

The zip manifest references additional `docs/`, `reference/`, `browser/`,
`api/`, `integration/`, and `scripts/` directories, but the archive received in
this checkout contains only top-level summaries and validation logs. The
validation logs themselves show the referenced browser module was missing in the
artifact environment. The repository implementation therefore derives its
requirements from the zip name, `SOURCE_BASIS.md`, and the current Arcweft
desktop/browser storage boundaries.

## Implemented Boundary

Native persistent file grants are now supported when an embedding host installs
host-opened persistent grant services:

- `arcweft-desktop-native` exposes `PersistentGrantConfig` and
  `PersistentGrantServices`.
- `NativeDesktopBuilder::with_persistent_grants` installs the services value.
- `NativeDesktopBuilder::try_build` reports provider load failures; `build`
  keeps the existing infallible builder surface for default hosts.
- `GrantStore` persists `GrantLifetime::Persistent` grants through the private
  services boundary, lazily restores persistent-format IDs as
  `GrantOrigin::Restored`, and removes persistent records on revoke.
- `DesktopFeature::PersistentFileGrant` remains unsupported without a provider
  and becomes `supported_with_user_consent` when a provider is installed.

Portable runtime/data crates remain Sans I/O. Native paths and platform
restoration details stay in `arcweft-desktop-native`, not in
`arcweft-desktop-contract`, `arcweft-core`, bundle, or save formats.

Browser storage is represented by a standalone ES module:

- `OpfsGrantStore`
- `IndexedDbGrantStore`
- `ApiGrantStore`
- `ReplicatedGrantStore`

The module lives at `web/arcweft-grant-storage.js` and is not wired into the
Wasm player bootstrap yet. It provides the multitarget storage surface described
by the package while preserving the existing browser player boundary.

## Non-Goals

This cut does not implement platform-specific macOS security-scoped bookmarks,
Windows credential vault persistence, Linux portal document-store tokens, or
browser UI prompts. Embedding hosts supply those storage details behind the
native provider or browser module.

The zip's missing OpenAPI and reference-workspace files were not reconstructed
as authoritative specs. Conditional API writes and tombstones are represented in
the browser `ApiGrantStore` request shape and replicated-store merge behavior.

## Verification

Commands run:

```bash
cargo fmt --all --check
cargo test -p arcweft-desktop-native
cargo test -p arcweft-desktop-contract -p arcweft-desktop-host -p arcweft-adapter-desktop
cargo test -p arcweft-adapter-desktop --test native_integration
cargo test -p arcweft-runtime-host
cargo clippy -p arcweft-desktop-native -p arcweft-desktop-contract -p arcweft-desktop-host -p arcweft-adapter-desktop --all-targets -- -D warnings
node web/tests/grant-storage-smoke.mjs
node --check web/arcweft-grant-storage.js
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-persistent-file-grants-2026-06-22
```

All commands above passed. The structural audit reported 0 errors and 88
warnings. The warning set and exact file metrics are recorded under
`docs/implementation/structure-audit-persistent-file-grants-2026-06-22/`.
