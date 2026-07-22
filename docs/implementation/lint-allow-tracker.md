# Rust lint `allow` tracker

## Purpose and authority

This is the single canonical tracker for Arcweft-owned Rust lint suppressions:
`#[allow(...)]`, `#![allow(...)]`, `cfg_attr(..., allow(...))`, workspace lint
policy exceptions, and command-line lint suppressions that target an Arcweft
manifest.  It exists to prevent repeated broad inventories from being mistaken
for progress.

Before starting an `allow` audit or cleanup, read this tracker first.  Reuse
the recorded baseline and inspect only the listed open rows, changes since that
baseline, or a trigger listed in **When to re-audit**.  Do not run a new
repository-wide subjective classification merely because a related Rust change
touches a file with an existing allow.

This tracker owns current status and scheduling.  The detailed, historical
enumeration and command output remain in
[`agents-policy-audit-response-2026-06-27.md`](agents-policy-audit-response-2026-06-27.md#f1-baseline-update-allow-usage-inventory-2026-07-21-historical-evidence).
The older external-search audit is historical context only:
[`agents-policy-audit-2026-06-27.md`](agents-policy-audit-2026-06-27.md).

## Recorded baseline

| Field | Recorded value |
| --- | --- |
| Audit date | 2026-07-21 |
| Working-copy revision | Jujutsu change `vszsuyoz` / Git commit `f427e35bb885a9374d56a3005d9f2bb969b5f65c` (parent `3acc9cfe`, `main`) |
| Tree state | Included unrelated in-progress work; the counts describe that checkout, not a clean commit. |
| Included scope | Checked-in Rust sources under the workspace, including unit and integration tests. |
| Excluded scope | `target/`, VCS data, `vendor/`, `third_party/`, docs, and generated/fixture/testdata/snapshot directories. |
| Method | Read-only static enumeration of complete Rust attributes, including direct and `cfg_attr`-mediated `allow`s. Full reproducibility detail is in the historical baseline note. |
| Validation boundary | This inventory did not newly run fmt, check, Clippy, tests, or the structure audit. Historical validation in the source note remains historical evidence only. |

### Measured counts

| Measure | Count |
| --- | ---: |
| `allow` attributes | 303 |
| Individual lint specifications | 348 |
| Outer attributes | 280 |
| Inner file/module attributes | 23 |
| `cfg_attr`-mediated attributes | 6 |
| Attributes with `reason = ...` | 202 |
| Attributes with only adjacent comment rationale | 36 |

The largest recorded sources were `arcweft-lang-sema` (94 attributes / 100
lint specifications), `arcweft-lsp` (46 / 51), `arcweft-lang-syntax` (32 / 34),
and `arcweft-core` (24 / 26).  The high-frequency lint groups were
`clippy::too_many_lines` (91), `clippy::too_many_arguments` (55), `dead_code`
(48), and `clippy::result_large_err` (48).

### Current nominal-publication cut delta

The accepted adapter/Rust nominal-publication cut was checked incrementally
against the recorded baseline rather than starting another broad
classification. Its net source delta adds exactly two Rust lint attributes:

- `arcweft-lang-sema/src/types/digest.rs::CanonicalTypeDigestEncoder::ty` —
  item-local `clippy::too_many_lines`, retaining one exhaustive fixed-tag
  `TypeKind` semantic-identity table; and
- `arcweft-lang-sema/src/types/digest.rs::CanonicalTypeDigestEncoder::entity_kind`
  — item-local `clippy::too_many_lines`, retaining one exhaustive fixed-tag
  `EntityKind` identity table.

Both carry local `reason` fields. The cut adds no inner/file-wide allow,
workspace policy exception, unsafe-boundary exception, or command-line
suppression. It passes
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.
These two entries are closed dispositions and do not add an open cleanup row;
revisit them only if the canonical identity encoding is split or replaced.

## Open inventory and disposition

These rows are the carried-forward classification from the 2026-07-21
baseline.  They are review priorities, not claims that the current code is
incorrect.  Paths and lines are baseline anchors and must be refreshed only
when the owning cut changes them.

| ID | Priority | Baseline location / scope | Disposition and required evidence |
| --- | --- | --- | --- |
| ALLOW-001 | P0 | `arcweft-desktop-native/.../windows_tsf/unsafe_com.rs:7`; inner `unsafe_code` | Audit each unsafe block inside its named Windows boundary and retain/add local safety rationale; do not broaden scope. |
| ALLOW-002 | P0 | `arcweft-lang-jit-cranelift/src/native_call.rs:3`; inner `unsafe_code` | Perform a JIT-boundary cut that proves containment and safety invariants; add a machine-readable/local rationale if retained. |
| ALLOW-003 | P1 | `arcweft-audio-mixer/src/effect.rs:1`; six inner Clippy allows | Replace file-wide scope with smallest justified item scopes or responsibility decomposition; review cast policy separately. |
| ALLOW-004 | P1 | `arcweft-audio-codec/src/lib.rs:2`; six inner Clippy allows | Same as ALLOW-003, but at crate-root scope; preserve only codec-format constraints with a local rationale. |
| ALLOW-005 | P1 | `arcweft-lang-hir/src/dialogue_application.rs:8`; inner `dead_code` | Decide whether the unused substrate is wired or removed; the forced-warning observation showed active dead-code diagnostics but was not a removal proof. |
| ALLOW-006 | P1 | `arcweft-lang-syntax/src/expr/dialogue_application.rs:6`; inner `dead_code`, `unused_imports` | Remove obsolete parser scaffolding or narrow to the smallest needed declaration; do not retain a historical parser path. |
| ALLOW-007 | P1 | `arcweft-lang-syntax/src/parser/document.rs:3`; inner `dead_code` | Reduce to declared fragment APIs or remove shadow parser pieces; existing reason does not justify module-wide suppression by itself. |
| ALLOW-008 | P1 | `arcweft-core/src/awbc/verify/code.rs:1`; inner `too_many_lines` | Resolve with the verifier structural-boundary review; split only on real responsibility boundaries. |
| ALLOW-009 | P1 | `arcweft-core/src/awbc/verify/structure.rs:1`; inner `too_many_lines` | Resolve with the same verifier structural-boundary review. |
| ALLOW-010 | P2 | `arcweft-lsp/src/profiles/accepted_project.rs:67`; item-local `dead_code` | Recheck when the accepted-project metrics migration closes; item-local reason is presently narrower than a file-wide exception. |
| ALLOW-011 | P2 | `arcweft-lsp/src/requests/signature.rs:322`; item-local `result_large_err` | Add a domain rationale or redesign error ownership if the error can be made smaller. |
| ALLOW-012 | P2 | `arcweft-core/src/awbc/vm.rs:179`; item-local `too_many_lines` | Review with the VM responsibility split; it is not a blanket file exception. |
| ALLOW-013 | P3 | root `Cargo.toml:285`; workspace `module_name_repetitions` | Decide and document whether domain naming needs a workspace-wide opt-out; do not add further global exceptions. |
| ALLOW-014 | P3 | root `Cargo.toml:286-287`; workspace `missing_errors_doc`, `must_use_candidate` | Decide whether public-API documentation policy warrants the global opt-outs. |

### Known non-Arcweft or conditional exceptions

- **ALLOW-015 — closed scope:** `just/verify.just:103` passes
  `-A clippy::too_many_arguments` only to the excluded vendored glyphon
  manifest. Recheck only if that command begins targeting an Arcweft manifest.
- The six recorded `cfg_attr` occurrences are covered by the owning module
  review: one browser-benchmark `dead_code` case and syntax incremental-code
  cases. They are not a separate global policy exception.
- The 2026-07-21 scan found no active non-vendor `-A lint`, `--allow lint`,
  `--cap-lints allow`, or `RUSTFLAGS` suppression in the scanned CI/configuration
  scope beyond the entries recorded above.

## Next work and timing

No lint cleanup belongs in the active nominal type-boundary integration.  After
that integration is validated and pushed, create an **independent allow-cleanup
cut** in this order:

1. ALLOW-001 through ALLOW-007: unsafe/file-scope/dead-substrate ownership.
2. ALLOW-008 and ALLOW-009: verifier structural boundary.
3. Item-local `too_many_lines`, `too_many_arguments`, and `result_large_err`,
   grouped by their owning responsibility rather than mechanically by lint.
4. ALLOW-013 and ALLOW-014: workspace lint-policy decision.

The cleanup cut must use direct compiler, behavior, and structural evidence;
it must not add source-text gates or compatibility shims merely to preserve an
obsolete suppression shape.

## When to re-audit

At the start of the independent cleanup cut, rerun the existing static
enumerator and compare it with this baseline.  Inspect only:

- the open IDs above;
- a changed path, line, lint, scope, or rationale for those IDs;
- newly introduced attributes or command-line suppressions; and
- rows whose listed next-cut evidence has become available.

A fresh full classification is warranted only after a workspace lint-policy
change, a new inner/file-scope allow, an unsafe-boundary change, or material
crate restructuring.  Record the new date, revision, scope, and delta in this
file, retaining the 2026-07-21 baseline rather than replacing it silently.
