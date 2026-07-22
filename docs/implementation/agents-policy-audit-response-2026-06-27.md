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

## F1 baseline update: `allow` usage inventory (2026-07-21; historical evidence)

The canonical current tracker and scheduling record is now
[`lint-allow-tracker.md`](lint-allow-tracker.md).  This section retains the
original measurement and detailed evidence; it is not the place to start a new
repository-wide reclassification.

This baseline superseded the former "inventory needed" wording when recorded,
but does not claim that every item below has been removed or individually
revalidated.

### Revision, scope, and reproducibility

- Audited working-copy revision: Jujutsu change `vszsuyoz`
  (`f427e35bb885a9374d56a3005d9f2bb969b5f65c`); its parent is `3acc9cfe`
  (`main`).  The working copy had unrelated in-progress changes, so the
  numbers describe the checkout, not a clean committed tree.
- Input: checked-in `*.rs` under the workspace, including unit and integration
  tests.  Excluded: `target/`, `.git/`, `vendor/`, `third_party/`, `docs/`, and
  directories named `generated/`, `fixtures/`, `testdata/`, or `snapshots/`.
  These are build output, VCS data, third-party material, historical/design
  documentation, or generated/test corpus rather than Arcweft implementation.
- Attribute occurrence means one source attribute that contains `allow`,
  including `cfg_attr(..., allow(...))`.  Lint-spec count means each lint name
  inside that `allow`; `reason = "..."` is metadata and is not a lint spec.
  An adjacent-comment count only includes consecutive `//` lines immediately
  preceding the attribute (it does not infer rationale from surrounding code).
- Static scan method (PowerShell; no source is written): enumerate with
  `rg --files -g '*.rs'`, apply the exclusions above, accumulate complete
  line-starting `#[...]`/`#![...]` attributes by bracket depth, then select
  direct `allow(...)` and `cfg_attr(... allow(...))`, remove the quoted
  `reason` argument before comma-splitting lint names, and group records by
  lint/crate/file.  A quick discovery command is:

  ```powershell
  rg -n -U --glob '!target/**' --glob '!.git/**' --glob '!vendor/**' `
    --glob '!third_party/**' --glob '!docs/**' --glob '!**/generated/**' `
    '^\s*#(?:!?)\[\s*(?:allow|cfg_attr)[\s\S]{0,500}?\ballow\s*\(' crates
  ```

### Measured baseline

| Measure | Count |
| --- | ---: |
| `allow` attributes | 303 |
| Individual lint specifications | 348 |
| Outer attributes | 280 |
| Inner (file/module-scope) attributes | 23 |
| `cfg_attr`-mediated attributes | 6 |
| Attributes with `reason = ...` | 202 |
| Attributes with only an adjacent `//` explanation | 36 |

Most frequent lint specifications were `clippy::too_many_lines` (91),
`clippy::too_many_arguments` (55), `dead_code` (48),
`clippy::result_large_err` (48), `clippy::cast_possible_truncation` (23), and
`clippy::cast_sign_loss` (18).  Other high-signal cases are `unsafe_code` (2),
`unused_imports` (1), and `private_interfaces` (1).

Largest attribute-bearing crates (attributes / lint specifications) were:
`arcweft-lang-sema` 94/100, `arcweft-lsp` 46/51,
`arcweft-lang-syntax` 32/34, `arcweft-core` 24/26,
`arcweft-lang-hir` 11/11, and `arcweft-runtime-plan` 10/10.  Largest files
were `arcweft-lsp/src/profiles/accepted_project.rs` (15/16),
`arcweft-lang-sema/src/character_definition.rs` (10/11),
`arcweft-lang-sema/src/callable/resolver.rs` (9/9), and
`arcweft-core/src/awbc/verify/code.rs` (6/6).

The active non-attribute suppressions are deliberately recorded separately:

- `Cargo.toml:285-287` allows `module_name_repetitions`, `missing_errors_doc`,
  and `must_use_candidate` workspace-wide.  `arcweft-desktop-native/Cargo.toml:54-56`
  repeats those Clippy allows while also denying unsafe code.
- `just/verify.just:103` invokes Clippy for the excluded vendored glyphon
  manifest with `-A clippy::too_many_arguments`; it does not suppress an
  Arcweft crate lint.  No active non-vendor `-A lint`, `--allow lint`,
  `--cap-lints allow`, or `RUSTFLAGS` suppression was found in the scanned
  configuration/CI/script scope.

### Observations versus conclusions

The following are observations.  A classification is a review priority, not a
claim that the current code is incorrect.  They apply the repository policy:
new allows should not be added casually; unsafe must be isolated; broad
file/module suppressions are to be reduced to the smallest justified scope.

| ID | Priority | Status | Path:line | Lint / scope | `reason` | Classification | Next cut / evidence needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ALLOW-001 | P0 | open | `crates/arcweft-desktop-native/src/text_input/windows_tsf/unsafe_com.rs:7` | `unsafe_code`; inner file scope | no | 正当化できそう | Windows-only module is explicitly named `unsafe_com`, but inspect every unsafe block and add/retain local safety rationale; do not widen scope. |
| ALLOW-002 | P0 | open | `crates/arcweft-lang-jit-cranelift/src/native_call.rs:3` | `unsafe_code`; inner file scope | no | 要精査 | Isolated JIT adapter is a plausible boundary, but the attribute has no machine-readable reason; verify containment and safety invariants in a JIT-focused cut. |
| ALLOW-003 | P1 | open | `crates/arcweft-audio-mixer/src/effect.rs:1` | six Clippy lints including `too_many_lines`; inner file scope | no | 要精査 | Replace file-wide allowance with item scopes or decomposition evidence; assess cast policy separately. |
| ALLOW-004 | P1 | open | `crates/arcweft-audio-codec/src/lib.rs:2` | six Clippy lints including `too_many_lines`; inner crate-root scope | no | 要精査 | Same broad-scope review; retain only codec-format constraints with a local rationale. |
| ALLOW-005 | P1 | open | `crates/arcweft-lang-hir/src/dialogue_application.rs:8` | `dead_code`; inner file scope | no | 削除候補 | Forced-warning compile emitted dead-code diagnostics for this file's types and methods.  This proves the allow currently suppresses active diagnostics, not that deletion is safe; decide whether to wire or delete the unused substrate. |
| ALLOW-006 | P1 | open | `crates/arcweft-lang-syntax/src/expr/dialogue_application.rs:6` | `dead_code`, `unused_imports`; inner file scope | no | 削除候補 | File-wide unused-import suppression is especially mechanical to remove after checking imports; keep no historical parser scaffolding silently accepted. |
| ALLOW-007 | P1 | open | `crates/arcweft-lang-syntax/src/parser/document.rs:3` | `dead_code`; inner file scope | yes | 要精査 | Reason exists, but inner scope suppresses all module diagnostics; reduce to the declared fragment APIs or remove obsolete shadow parser pieces. |
| ALLOW-008 | P1 | open | `crates/arcweft-core/src/awbc/verify/code.rs:1` | `clippy::too_many_lines`; inner file scope | yes | 正当化できそう | Verifier cohesion may justify a temporary file scope, but the 6 local allows make this a structural-audit follow-up; split only at responsibility boundaries. |
| ALLOW-009 | P1 | open | `crates/arcweft-core/src/awbc/verify/structure.rs:1` | `clippy::too_many_lines`; inner file scope | yes | 正当化できそう | Same verifier review; use the structural audit rather than a text/source gate. |
| ALLOW-010 | P2 | open | `crates/arcweft-lsp/src/profiles/accepted_project.rs:67` | `dead_code`; outer item scope | yes | 正当化できそう | Item-local and reasoned; recheck when the accepted-project metrics migration closes. |
| ALLOW-011 | P2 | open | `crates/arcweft-lsp/src/requests/signature.rs:322` | `clippy::result_large_err`; outer item scope | no | 要精査 | Local scope is good, but add domain reason or redesign error ownership if the result error is avoidably large. |
| ALLOW-012 | P2 | open | `crates/arcweft-core/src/awbc/vm.rs:179` | `clippy::too_many_lines`; outer item scope | no | 要精査 | Review with VM responsibility split; not a blanket file suppression. |
| ALLOW-013 | P3 | open | `Cargo.toml:285` | `module_name_repetitions`; workspace-wide lint policy | n/a | 要精査 | Policy-level allow is broad; retain only after documenting why domain type names need it across crates. |
| ALLOW-014 | P3 | open | `Cargo.toml:286-287` | `missing_errors_doc`, `must_use_candidate`; workspace-wide lint policy | n/a | 要精査 | Decide whether public API/documentation policy warrants global opt-out; do not add further global exceptions. |
| ALLOW-015 | P3 | closed-scope | `just/verify.just:103` | `-A clippy::too_many_arguments`; vendored glyphon command | n/a | 正当化できそう | Excluded third-party manifest only; recheck only if this command starts targeting an Arcweft manifest. |

`cfg_attr` occurrences (six) are not silently ignored: browser benchmark
`dead_code` is target-conditional at
`crates/arcweft-browser-bench/src/correctness.rs:1`; the remaining conditional
dead-code cases are in syntax incremental code.  They remain covered by the
same open review of their containing module rather than by a separate global
exception.

### Validation boundary and next audit

The earlier workspace Clippy pass recorded in this note's **Validation**
section remains historical evidence only.  This baseline did **not** newly run
`cargo fmt`, workspace `cargo check`, workspace Clippy, tests, or the structure
audit.  One exploratory forced-warning command was run before the baseline was
written:

```powershell
$env:RUSTFLAGS='-D warnings --force-warn dead_code'
cargo check -p arcweft-lang-hir --all-targets
```

It exited 0 and emitted dead-code warnings for
`src/dialogue_application.rs`, supporting ALLOW-005's observation.  Because
the command also forces warnings in dependencies, it is not a clean workspace
Clippy result and must not be represented as one.

Repair order: (1) after the active Lang-01.1.1.2.2 nominal-publication cut is
validated and pushed, handle ALLOW-001 through ALLOW-007 as an independent
cleanup cut; (2) review ALLOW-008/009 with the verifier structural boundary;
(3) handle item-local `too_many_lines`, `too_many_arguments`, and
`result_large_err` grouped by their owning responsibility; (4) review the
workspace policy rows ALLOW-013/014.  Do not mix mechanical allowance removal
with an unrelated language or runtime contract cut.

On the next audit, do **not** repeat an all-item subjective review.  Re-run the
static enumerator, compare its records against this table, and inspect only:
open IDs, changed `path:line`/lint/scope/reason fields, newly introduced
attributes or command-line suppressions, and rows whose next-cut evidence is
now available.  A full reclassification is required only after a workspace
lint-policy change, a new inner/file-scope allow, an unsafe-boundary change, or
a material crate restructuring.

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
