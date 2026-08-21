# Verification scope and implementation gates

## Actually inspected for this return

- Private repository `Sanzentyo/arcweft` through the GitHub connector.
- Current `origin/main` full SHA `4bda1cdcea63fdf7aac32691d756c1c0e1fc693e`.
- The source-equivalent baseline `dec4f6c2de3be87d28a2f976b1ae51e3b40dd3fd` and the docs-only delta to current main.
- Root and applicable nested `AGENTS.md` files through the end, current docs/review rules, crate map, request, implementation blocker, retained parent request/correction/frozen mirror, final-HIR View parent, and current source owners listed in `SOURCE_EVIDENCE.csv`.
- The generated package file set, CSV counts/classes, required text decisions, version markers, exact SHA, manifest, hashes, deterministic ZIP path set, extraction, and machine validation.

## Not claimed by this design-only return

No production source, test, fixture, manifest, branch, patch, or implementation overlay was changed. No Rust build was run against a repository checkout. Therefore this archive does not claim that the future implementation already passes compilation, Clippy, nextest, docs, AOT, parity, save/replay, or Tier-2 gates.

## Required implementation commands/gates

Repository instructions and the test matrix require the implementation to close at least:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
python3 <package>/tools/validate_package.py <package>
```

The repository's maintained Tier-2/AOT/generated-schema commands take precedence where they prescribe a stricter invocation. Platform rows must run on Linux, Windows, and macOS; VM/AOT differential tests must compare selector cases/bindings and Need handle behavior byte-for-byte or by canonical digest as specified.

## Archive verification statement

The ZIP documents the exact extent of source inspection and separates it from unexecuted implementation gates. Its internal manifest covers every payload. `VALIDATION.json` records package-level checks only and does not mislabel design validation as production implementation validation.
