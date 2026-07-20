# AGENTS policy audit response - 2026-06-27

This note records the local follow-up to
`docs/implementation/agents-policy-audit-2026-06-27.md` after pulling
`origin/main` on 2026-06-27.

## Pulled audit source

- Latest `main` after fetch/rebase: `7fd9668764068c13912ccec1277936078cdb3652`
  (`Document AGENTS policy audit findings`).
- Local working copy was clean before applying fixes.
- The pulled change added the audit memo only; it did not change production
  code.

## Applied in this cut

The following findings are narrow enough to fix without redesigning runtime or
parser boundaries:

- F2: remove backend/adapter-domain empty feature names from
  `arcweft-core`.
- F5: rename `arcweft-render-wgpu` numeric conversion helpers from endpoint
  type names to domain policy names while preserving behavior.
- F6: rename the optional bincode boundary from compatibility/legacy wording
  to explicit external interop wording, without adding compatibility aliases.
- Web parity fixture: regenerate `web/demo.awfb` from `web/src/main.arcw` after
  the pulled audit exposed a stale checked-in AWFB in `just test-workspace`.
- CLI stdio MCP test stability: replace a fixed sleep in the stderr-tail
  timeout test with a bounded wait for the child stderr reader to observe the
  expected tail marker.
- Documentation gates: remove a host-specific attachment path from the
  seq-02.3/02.4 implementation note and reword the pulled audit memo so the
  regression harness does not see removed compatibility-layer wording outside
  `docs/reviews`.

Residual search hits for the old F5/F6 names are expected only in the original
audit memo or in unrelated crates that own separate local conversion helpers.
The `arcweft-render-wgpu` F5 helper names and `arcweft-codec-binary` F6 names
were removed from production code.

## Classified but not implemented in this cut

F1 (`#[allow]` inventory) is repository-wide and includes production, tests,
generated-source gates, intentional precision casts, and large legacy verifier
or renderer functions. The local scan command was:

```bash
rg -n --glob '*.rs' '#!?\[allow|#!\[feature|unsafe\s*\{|unsafe fn|Box::leak|mem::forget' crates tools tests
```

No `Box::leak` or `mem::forget` production hits were found by this scan.
`unsafe` hits are concentrated in `arcweft-lang-jit-cranelift`, where the crate
is an isolated JIT boundary rather than low-level core code. The remaining
`#[allow]` set is too broad for a drive-by edit; each item needs an owner
classification and a removal plan or local rationale.

F3 (core audio/capture command boundary) requires a design split. The current
implementation is Sans I/O typed request lowering: it evaluates runtime
expressions into `arcweft-interaction-model` audio request data and does not
open devices, query microphones, access clocks, or touch platform APIs. Moving
that boundary to a different crate should be done as a dedicated runtime/audio
ownership migration, not as part of the policy-name cleanup.

F4 (parser `split_top_level` / logical item residuals) is a parser architecture
change. The local scan found existing call sites both inside syntax parsing and
outside the syntax crate, including agent label parsing. Replacing
line-oriented block collection with CST block item events needs grammar
fixtures and parser/HIR/sema validation as its own task.

F7 (`task_requests` / `line_effects` vocabulary) currently appears in metrics,
reporting, runtime host summaries, and CLI output rather than as obsolete
`RuntimeStepOutput` fields. Runtime semantic fields already use
`RuntimeEffectBatch` and `HostRequestBatch`. Any JSON/report rename would be a
user-facing output schema change and should be handled as an explicit reporting
schema migration.

## Follow-up requests

- Create a dedicated F1 allow-inventory cleanup task that classifies each
  production allow by owner, then removes or documents the smallest item scope.
- Create a dedicated F3 runtime/audio ownership design before moving
  `RuntimeAudioCommand` out of core.
- Create a dedicated F4 CST event migration design before replacing parser
  logical item scans.
- Create a dedicated F7 reporting schema migration if CLI/runtime-host JSON
  should rename `task_requests` or `line_effects`.

## Validation

Passed after fixes:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-codec-binary --all-targets --all-features
cargo test -p arcweft-render-wgpu --all-targets --all-features
cargo test -p arcweft-player-web --test parity --all-features -- --nocapture
cargo test -p arcweft-cli app::agent::mcp_stdio::tests::stdio_transport_times_out_and_retains_bounded_stderr_tail --lib -- --nocapture
cargo test -p arcweft-cli --test regression_harness -- --nocapture
cargo run -p arcweft-cli --quiet -- inspect web/demo.awfb --json
cargo tree -e features -p arcweft-core
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

The structure audit reported `0 error(s), 106 warning(s)`, matching the known
warning-level state.
