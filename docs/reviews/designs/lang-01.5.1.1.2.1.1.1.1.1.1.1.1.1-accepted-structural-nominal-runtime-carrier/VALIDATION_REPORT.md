# Validation report

Date: 2026-08-23

Overall: PASS

## Repository preflight

- HEAD and origin/main: 9a5d30d25620541c3f2975d31e04e04e3bc9514c
- production paths under Cargo.toml, Cargo.lock, and crates/: clean
- cargo metadata --no-deps --format-version 1: PASS
- exact maintained request SHA-256:
  90ca32e38481fdf152b9ff5aaf145b4514b15ece7a92e989588adaa9b9481fbf
- REQUEST.md byte identity: PASS

## Design validation

- positive validator: PASS
- negative self-tests: PASS (15 independent mutations)
- rustfmt for both Rust scripts and shared support: PASS
- manifest membership and SHA-256 rows: PASS
- required files/status/open questions/schema/decision/cut/wire gates: PASS
- source blob and Cargo dependency-direction gates: PASS
- git diff --check on the design folder: PASS

The first positive-validator attempt did not compile because the included
support file began with an inner doc comment (`//!`, Rust E0753). It made no
repository mutation. The comment was changed to an ordinary module comment,
the manifest was regenerated, and the complete positive and negative suites
then passed as recorded above.

An independent root recheck later found import-order drift in
`tools/validate_design.rs` and `tools/negative_self_tests.rs`. The imports were
formatted, their manifest rows were regenerated, and the positive validator,
all 15 negative mutations, `rustfmt +nightly --check`, and design diff check
were rerun and passed.

Commands:

    cargo +nightly -Zscript tools/validate_design.rs --repository-root D:/git/arcweft
    cargo +nightly -Zscript tools/negative_self_tests.rs
    rustfmt +nightly --check tools/validation_support.rs tools/validate_design.rs tools/negative_self_tests.rs
    git diff --check -- docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier

## Intentionally not run

Production Cargo check/tests and runtime fixtures were not run because this
cut changes documentation and read-only design validators only. Those tiers
are mandatory in the implementation cuts.

No production Rust, Cargo file, test, fixture, frozen package, ZIP, branch,
worktree, commit, or push was changed by this design task.
