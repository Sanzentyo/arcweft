# Persistent File Grants Redesign ce9ea4, 2026-06-23

This note records the integration of
`arcweft-persistent-file-grants-redesign-ce9ea4.zip` against the current
checkout.

## Equivalence Check

The previous `main` contained a persistent grant hook, but it was not equivalent
to this redesign package. The missing parts were contract-owned generated ID
semantics, supplemental feature requirements for persistent-producing requests,
private native persistent grant services, lazy restore on persistent-format
cache miss, and durable persistent revoke routing.

## Implemented

- Added contract-owned methods for `GrantLifetime`, `FileGrantId`,
  `GrantOrigin`, `GrantAccess`, `FileEntryKind`, `FileDialogMode`,
  `UserFileRequest`, and `DesktopRequest`.
- Replaced the native public `PersistentGrantStore` trait and public record/root
  types with `PersistentGrantConfig` and `PersistentGrantServices`.
- Moved stored-record shape and restore metadata behind the private
  `arcweft-desktop-native::persistent_grants` module.
- Updated `NativeDesktopBackend` to check every feature from
  `DesktopRequest::required_features`, so persistent dialogs and known-directory
  requests require both their primary feature and `PersistentFileGrant`.
- Updated `GrantStore` to generate persistent IDs with the contract prefix,
  retain issued permission provenance separately from public
  `GrantOrigin::Restored`, restore only persistent-format cache misses, and
  route persistent revocation through the services boundary even when the grant
  is not cached.
- Kept the default backend fail-closed: `PersistentGrantServices::open` returns
  `Unsupported(PersistentFileGrant)` until a live platform authority is reviewed
  and validated.

## Boundaries Kept

- No new public sealed-token workspace crate was added.
- No public `TokenKey`, master-key provider trait, platform-provider trait, or
  runtime grant enumeration was added.
- Portable contract/core/runtime crates still do not store native paths or
  platform authority material.
- OS authority providers for DPAPI, Keychain/bookmarks, and XDG Documents
  portal/Secret Service remain explicit follow-up work before production
  capability advertisement outside tests.

## Verification

Commands run so far:

```bash
cargo fmt --all --check
cargo test -p arcweft-desktop-contract
cargo test -p arcweft-desktop-native
cargo test -p arcweft-desktop-contract -p arcweft-desktop-host -p arcweft-adapter-desktop
cargo check --workspace --all-targets
cargo clippy -p arcweft-desktop-contract -p arcweft-desktop-native -p arcweft-desktop-host -p arcweft-adapter-desktop --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-persistent-file-grants-redesign-ce9ea4-2026-06-23
git diff --check
```

The structural audit reported 0 errors and 88 warnings. The warning set and
file metrics are recorded under
`docs/implementation/structure-audit-persistent-file-grants-redesign-ce9ea4-2026-06-23/`.
