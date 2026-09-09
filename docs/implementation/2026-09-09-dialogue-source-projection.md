# Ruby source projection and generated expression admission — 2026-09-09

Inspected base: `fc6a49dd530343246741c1cf4530f3ecf39796fc`, existing `main`,
pushed and clean before this cut. Supersedes the unresolved HIR publication
failure in the [prepared call storage record](2026-09-09-prepared-call-node-storage.md).

## Reproduction and selected authority

The native observe fixture `alice: Hello |[夢](ゆめ)[r][voice auto][p]`
failed with `hir.lower.project_publish` / `InvalidSourceIndex`. A direct CLI
matrix isolated the Ruby abbreviation: plain text, line break, voice and page
controls passed individually; Ruby alone and Ruby plus the controls failed.

The maintained [Ruby contract](../01-language/converged-language-surface.md)
retains both `|[夢](ゆめ)` and `｜夢《ゆめ》` alongside
`#ruby("ゆめ")[夢]`. All three converge on the ordinary typed content call.
There is no new HIR Ruby node or parallel semantic reader.

The source query family now admits the retained Ruby role on a content-call
node, while its immutable source manifest still decides which roles actually
exist. A canonical hash call does not acquire a Ruby span, and an abbreviation
does not acquire a hash/header span. Authored nested-expression cardinality is
derived from the typed syntax projection; generated content calls are not
counted as authored children.

`HirRubyDesugaring` owns the ephemeral construction recipe for the target,
reading and content application. Lowering and freeze validation share its
synthetic keys and exact HIR payloads. Validation checks the target path,
argument, body, scope, parent and ordinal identities, and source-site
provenance. The same admission runs for source-backed expressions and both
levels of candidate-only dialogue expressions. A validation-local inventory
must equal all published Ruby-generated slots, rejecting unreferenced leaves.
No second persisted catalog is introduced. The former lowering-only
construction and its one-use allocation helper are deleted; reused slots
must match the complete expected payload.

## Validation

Logs are local and ignored under
`.arcweft-local/validation/2026-09-09-dialogue-source-projection/`.

- Full-project Ruby source tests: 2 passed, covering both retained forms,
  canonical hash form, exact source-role spans, inapplicable roles and stale
  revision rejection (`hir-shared-desugaring.log`).
- Final generated-expression tests: 4 passed, covering direct content, nested
  hash content, an ambiguous closure/index versus Ruby/text interpretation,
  and a block inside the index candidate. Nine valid-shape payload tamper
  cases, two orphan-reading cases and one scope substitution are rejected
  before module publication (`hir-contexts-final.log`).
- Final `cargo test -p arcweft-lang-hir --lib -- --nocapture`: 892 passed,
  0 failed, 8 ignored; 160.89 s including rebuild (`hir-all-final.log`).
- First full HIR library run: 889 passed, 3 failed, 8 ignored; 165.41 s including
  build. The failures were the added test fixtures, not existing tests: text
  inside a quoted literal did not generate a Ruby graph. Those fixtures were
  replaced with a real two-candidate surface, and all four focused tests then
  passed. Earlier fixture probes also incorrectly expected the alternative
  dialogue interpretation of a block to be clean; the accepted test explicitly
  retains that interpretation's recovered point-action state.
- `cargo fmt --all`: passed initially in 10.48 s and after test lint cleanup
  in 10.40 s.
- `cargo check --workspace --all-targets --all-features`: passed with existing
  warnings, 24.06 s.
- `cargo clippy --workspace --all-targets --all-features`: passed in 31.22 s;
  two warnings in the added source test were corrected, then the final run
  passed with existing warnings in 8.13 s. No lint suppression was added.
- `just test-tier2`: MCP 4 passed; native observe 0 passed / 1 failed;
  72.25 s including rebuild. HIR publication now succeeds and native capture
  reaches a different error: `object.dialogue.0.0.ruby.0` matches two
  prepared-text owners. The recipe stopped there; auxiliary capture,
  visual-golden and production proof stages were not run.
- `just test-workspace`: 856 passed / the same 18 callable failures across
  84 reports, 284.12 s. The recipe stopped in compiler `callable_execution`;
  later workspace suites and the recipe's subsequent CLI commands were not
  run. The separate final HIR run above supplies the changed-crate coverage.
- `just test-doc`: 95 reports, 8 passed, 0 failed; 50.03 s.
- Structural audit with `--fail-on-blocking`: 95 packages, 2,247 Rust files,
  309 review triggers, 0 blocking violations; 2.44 s.
- Documentation links: 2 files / 30 local targets passed; anchors not checked.
  An initial one-off scan falsely treated Ruby examples inside code spans as
  links; the corrected scan excludes code spans. `git diff --check` passed.

The first probe build failed because a test used unstable `str::as_str` on
boxed text; it was corrected to stable `as_ref`. No unstable feature was
enabled. The first canonical source test incorrectly included the attached
body in its header span expectation; the final test asserts the actual
authored header separately from the whole node.

Commands ran sequentially with Cargo's normal concurrency and existing feature
combinations. No Cargo job count, stack limit or test profile was changed.
The 8 ignored HIR tests were not enabled by the library command. This cut does
not claim success for the known callable or remaining Tier 2 failures.

## Ownership review

The audit measured this cut dirty against the inspected base. Every path below
is relative to `crates/arcweft-lang-hir/src/`, owned by `arcweft-lang-hir`, with
zero embedded test LOC in the production files. HIR's normal dependency
fan-in/out is 10/3; development fan-in/out is 1/0. No manifest, dependency,
feature or public export changes.

| Path | Classification | Base → current physical LOC | Bytes |
| --- | --- | ---: | ---: |
| `dialogue_application.rs` | Production | 1,031 → 1,032 | 36,837 |
| `dialogue_application/ruby.rs` | Production | New → 125 | 4,871 |
| `final_lowering/expression_lowering/dialogue.rs` | Production | 998 → 909 | 39,196 |
| `final_lowering/expression_lowering/tests.rs` | Test | 3,141 → 3,143 | 112,229 |
| `final_lowering/expression_lowering/tests/dialogue_desugaring.rs` | Test | New → 166 | 7,040 |
| `final_lowering/tests.rs` | Test | 517 → 519 | 18,754 |
| `final_lowering/tests/dialogue_sources.rs` | Test | New → 118 | 4,588 |
| `source_index/expr_projection.rs` | Production | 958 → 962 | 37,470 |
| `source_index/expression_manifest.rs` | Production | 1,395 → 1,416 | 61,464 |
| `source_index/expression_manifest/projection.rs` | Production | 1,264 → 1,262 | 49,094 |
| `source_index/expression_manifest/candidate_projection.rs` | Production | 1,394 → 1,413 | 57,854 |
| `source_index/expression_manifest/candidate_projection/payload.rs` | Production | 479 → 491 | 19,396 |
| `source_index/expression_manifest/desugaring.rs` | Production | New → 85 | 2,990 |

The three source-manifest owners above the production review trigger retain
their existing responsibilities: immutable source-row admission, exhaustive
authored payload/child projection, and candidate graph/descendant validation.
The new generated-graph validation is extracted at its actual shared boundary;
both source and candidate admission call it. Candidate descendant identities
remain separate from generated expression identities during validation, then
the final slot inventory closes both ownership domains. There is no new
runtime state, I/O or cross-layer dependency in these owners.

The construction recipe belongs to HIR's dialogue application domain, not to
the parser or the source index. Its visibility stays crate-private so the
lowerer and validator can share it without adding a facade API. The large
expression test owner only registers a child module; new tests follow the
generated graph and full-project source boundaries in separate files. This
is a cohesion disposition for the touched review triggers, not an assertion
that file size alone establishes correctness. Generated metrics remain local.

## Remaining work

Native capture's prepared-text ownership ambiguity is the next runtime
failure, not a remaining HIR publication error. A separate read-only fixture
also exposed `alice()[|[夢](ゆめ)]`: the index interpretation contains a
recovered closure with a missing terminator/body and fails earlier during
candidate lowering with `InvalidArenaCommit`. The ordinary colon form and
bracket content with preceding text reach the repaired Ruby boundary. The
recovered candidate failure remains required follow-up; no supported surface
is removed or declared exempt.

The known sema 7 and callable 18 failures and all remaining
Match/View/task-plan/nominal/scheduler acceptance stay in the active
[convergence goal](2026-09-08-convergence-goal-plan.md). This cut introduces no
design deviation or contract-version change and does not complete that goal.
