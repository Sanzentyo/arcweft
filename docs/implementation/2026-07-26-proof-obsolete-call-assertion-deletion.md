# Proof obsolete call-shaped assertion deletion

Date: 2026-07-26

## Scope

This deletion-driven cut removes an obsolete runtime-plan success path that
recognized ordinary calls named `assert` or `debug_assert` from callee strings
and converted them into runtime assertions. That path supplied a default
message, had no typed guard identity, and bypassed the retained typed
`assert.check` / `assert.debug` statement authority.

Both the evaluated-effect and static-control dispatch branches are removed,
together with the private `runtime_assertion` helper and the old success
integration test. Closed ordinary calls using those names now remain generic
host calls; a runtime-valued ordinary call remains subject to the existing
generic typed-boundary error. Neither name can construct
`RuntimeEffectExpr::Assert` or `LineEffectRequest::Assert` through ordinary-call
lowering. A direct effect-lowering unit test records that observable boundary
without a removed-syntax diagnostic or source gate.

The line-plan assertion item, `ensure`, and the typed
`assert.check` / `assert.debug` statement path are separate retained constructs
and are unchanged.

## Deferred atomic boundary

Mandatory guards on core `RuntimeAssertion`, typed assertion sites and
inventories, and the AWBC assertion codec remain one later atomic switch. They
must consume the final typed `StmtId`/`ExprId` and callable context. This cut
does not invent a zero, optional, random, source-derived, or message-derived
guard merely to keep the deleted ordinary-call path compiling.

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
