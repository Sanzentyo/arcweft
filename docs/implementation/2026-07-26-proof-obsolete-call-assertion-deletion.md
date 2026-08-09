# Proof obsolete call-shaped assertion deletion

Date: 2026-07-26

## Scope

This deletion-driven cut removes an obsolete runtime-plan success path that
recognized ordinary calls named `assert` or `debug_assert` from callee strings
and converted them into runtime assertions. That path supplied a default
message, had no typed guard identity, and bypassed the retained typed
`assert.check` / `assert.debug` statement authority.

Both the evaluated-effect and static-control dispatch branches were removed,
together with the private `runtime_assertion` helper and the old success
integration test. The later Proof public switch also deletes the semantic
`BuiltinCallableId::Assert` / `BuiltinCallableId::DebugAssert` registrations,
so those spellings receive no built-in ordinary-call candidate. An explicitly
declared callable with the same ordinary name still follows normal project
name resolution; there is no implicit host fallback or assertion meaning.

The later switch also deletes `LineTaskGroup::assertions` and
`LineAssertionRequest`. The sole language runtime-assertion route is now the
typed `assert.check` / `assert.debug` statement, its checker-owned semantic
admission, the accepted runtime assertion site/inventory, and canonical AWBC
lowering. `assert.prove` remains verification-only and cannot acquire a runtime
mode or guard. `ensure` is an unrelated ordinary language callable.

## Completed atomic successor boundary

The subsequent Proof public-switch working copy implements the formerly
deferred boundary with mandatory typed guards, session-only assertion sites
and inventories, the canonical AWBC assertion payload, and persisted artifact
fingerprints. It consumes final typed `StmtId` / `ExprId` and callable context;
it does not derive identity from a source string, range, message, zero value,
or random seed. Exact validation evidence is recorded in
[Proof-concurrency v6.1.1 runtime assertion closure](2026-08-06-proof-concurrency-v6-1-1-runtime-assertion-closure.md)
and the base-package matrix ledger.

## Validation

The isolated revision passed:

```text
cargo fmt --all -- --check
  PASS
cargo test -p arcweft-runtime-plan expr::effect::tests --lib --all-features
  PASS, 6 passed
cargo check -p arcweft-runtime-plan --lib --all-features
  PASS
cargo clippy -p arcweft-runtime-plan --lib --all-features
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
  PASS
cargo test -p arcweft-cli --lib --bins --quiet
  PASS, 197 passed
cargo test -p arcweft-cli --test runtime_native_options --quiet
cargo test -p arcweft-cli --test check_core_cli --quiet
cargo test -p arcweft-cli --test native_style_parity_sample --quiet
cargo test -p arcweft-cli --test release_trust_json --quiet
cargo test -p arcweft-cli --test responsive_stage_placement --quiet
cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens --quiet
  PASS
cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
  KNOWN FAIL, 3 passed and 2 failed: capability-owned FsError nominal
  publication remains blocked on the Proof public HIR switch
cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS, 3,675 files, 1,936 Rust files, 906,061 Rust physical LOC,
  94 package manifests, 0 errors, 146 repository-wide warnings
git diff --check
  PASS
```

The first broad-test attempts were also constrained by local validation
resources: a clean worktree needed the repository-ignored Japanese font
fixture copied from the primary workspace, and parallel test linking first hit
Windows `os error 1455`. Re-running with the same feature set and
`CARGO_BUILD_JOBS=1` produced the results above. Neither condition changed the
checkout or the asserted behavior.

This narrow deletion changes no crate dependency, persisted codec, public
contract, runtime host, rendering, Agent, MCP, or capture path. It does not
trigger Tier 2 or a new structural audit.
