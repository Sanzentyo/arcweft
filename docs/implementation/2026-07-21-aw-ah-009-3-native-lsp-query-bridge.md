# AW-AH-009.3 native LSP signature-query bridge

Date: 2026-07-21

## Status

Implementation and focused validation are complete in isolated Jujutsu change
`ymkunuwq` at:

```text
D:\git\arcweft-ws-aw-ah-009-3-gap1
```

The change is based on `main` commit `84f9574c` and is not integrated. It closes
only gap 1 of the request-path handoff from
`arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip`; it
does not claim completion of the parent AW-AH-009.3 acceptance matrix.

An admitted LSP signature request now invokes
`arcweft_lang_sema::signature::query_signature` against its exact accepted
document, canonical HIR module, and registered semantic world. The request
worker publishes either the typed result or typed failure only after the
existing complete final-stamp validation.

## Implemented boundary

- `SemanticSignature` carries authored and canonical callee fields instead of a
  preformatted presentation label. There is no compatibility getter or second
  label carrier.
- The LSP layer builds the display label exactly once from the authored callee,
  typed parameter groups, and typed result.
- Parameter label ranges are accumulated in checked UTF-16 code units and use
  `ParameterLabel::LabelOffsets`; no rendered-label parser is involved.
- The request path converts the negotiated LSP position through
  `LineIndex::try_byte_offset_from_position`.
- The admitted request's cancellation flag and deadline are borrowed by the
  native semantic query.
- `SignatureQueryOutcome::NotApplicable` becomes an LSP `null` result.
- Checked position, semantic query, cancellation, deadline, stale request,
  acquisition, and projection failures map by typed variant to protocol codes
  and structured `ResponseError.data.code`.
- The final request stamp is revalidated immediately before publishing either a
  successful result or an error.
- The word-based Rust-metadata resolver, entry-role signature resolver,
  synchronous session signature branch, signature-only word helper, and
  verifier-LSP `rust_adapter_signature_help` API are deleted.
- Windows extended drive paths are normalized by the profile path/URI owner to
  the ordinary client spelling `file:///C:/...`. Extended UNC paths retain a
  real UNC authority such as `file://server/share/...`; no URI alias or dual
  lookup was added.
- The production request fixture loads the current manifest shape, registers
  the standard `inference-tensor` adapter, publishes a real accepted
  environment, acquires its ordinary lease, and requests
  `infer.add_f32(value, value)`.

## Owned files

Production ownership:

```text
crates/arcweft-lang-sema/src/callable/facts.rs
crates/arcweft-lang-sema/src/signature/project.rs
crates/arcweft-lsp/src/features/entry_roles.rs
crates/arcweft-lsp/src/features/hover.rs
crates/arcweft-lsp/src/features/signature.rs
crates/arcweft-lsp/src/profiles/uri.rs
crates/arcweft-lsp/src/requests/executor.rs
crates/arcweft-lsp/src/requests/signature.rs
crates/arcweft-lsp/src/server.rs
crates/arcweft-lsp/src/session.rs
crates/arcweft-lsp/src/session/signature.rs
crates/arcweft-verify-lsp/src/lib.rs
```

Focused test ownership:

```text
crates/arcweft-lang-sema/src/callable/tests.rs
crates/arcweft-lang-sema/src/signature/tests.rs
crates/arcweft-lsp/src/features/entry_roles/tests.rs
crates/arcweft-lsp/src/features/signature.rs
crates/arcweft-lsp/src/profiles/uri.rs
crates/arcweft-lsp/src/requests/signature.rs
crates/arcweft-lsp/src/session/tests.rs
crates/arcweft-verify-lsp/src/lib.rs
```

The sema files own only the native result-shape correction. LSP owns protocol
presentation, request admission/execution/publication, and path/URI
normalization. `arcweft-verify-lsp` only loses its competing legacy resolver.

## Explicit non-goals

- The typed bounded signature cache and its invalidation matrix remain the
  separate AW-AH-009.3 cache cut.
- Semantic overload selection, optional active-signature correction,
  diagnostics truncation, and production work/limit reconciliation remain the
  separate semantic-result/resource-accounting cut.
- This cut does not add or redesign CharacterDialogue callable families. It
  consumes accepted shared-resolver facts and does not restore removed `.say`
  or colon dialogue surfaces.
- No compatibility shim, deprecated re-export, source gate, word resolver,
  label parser, source-text fallback, URI alias, or dual lookup is present.
- Completion, hover, rename, stable design chapters, manifests, and schemas are
  not redesigned.

## Integration boundary

The isolated change was rebased first onto `6b9132f6`, then onto
`84f9574c`, and finally onto `main` `240f5bc8`; every rebase completed with
zero conflicts. No branch or bookmark was created. The integrated change is
the direct descendant `0015a21b`.

## Structural audit

The canonical audit was run from change `ymkunuwq`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Result:

```text
files scanned: 3413
Rust files: 1776
Rust physical LOC: 816293
package manifests: 93
violations: 0 error(s), 131 warning(s)
```

All warnings are repository-wide existing ownership warnings. This cut adds no
Cargo dependency or feature edge. Current dependency counts are:

| Crate | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-lang-sema` | 11 | 12 |
| `arcweft-lsp` | 1 | 29 |
| `arcweft-verify-lsp` | 1 | 10 |

Changed Rust file measurements:

| Path | Class | Bytes | Physical LOC | Embedded test LOC | Responsibility in this cut |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | production | 30,239 | 946 | 0 | Structured semantic result fields |
| `crates/arcweft-lang-sema/src/callable/tests.rs` | unit test | 65,566 | 1,945 | 0 | Result invariant fixture |
| `crates/arcweft-lang-sema/src/signature/project.rs` | production | 13,194 | 371 | 0 | Authored/canonical callee projection |
| `crates/arcweft-lang-sema/src/signature/tests.rs` | unit test | 27,713 | 882 | 0 | Alias and typed callee evidence |
| `crates/arcweft-lsp/src/features/entry_roles/tests.rs` | unit test | 22,716 | 729 | 0 | Competing signature assertions removed |
| `crates/arcweft-lsp/src/features/entry_roles.rs` | production | 19,701 | 531 | 0 | Competing resolver removed |
| `crates/arcweft-lsp/src/features/hover.rs` | production | 34,072 | 1,002 | 384 | Signature-only word wrapper removed |
| `crates/arcweft-lsp/src/features/signature.rs` | production | 8,767 | 253 | 73 | Native semantic-to-LSP projection |
| `crates/arcweft-lsp/src/profiles/uri.rs` | production | 4,837 | 159 | 29 | Drive/UNC client URI normalization |
| `crates/arcweft-lsp/src/requests/executor.rs` | production | 6,756 | 213 | 0 | Native query worker invocation |
| `crates/arcweft-lsp/src/requests/signature.rs` | production | 29,493 | 803 | 85 | Lease/stamp carriers and typed errors |
| `crates/arcweft-lsp/src/server.rs` | production | 7,463 | 179 | 0 | Native request intake and typed admission response |
| `crates/arcweft-lsp/src/session/signature.rs` | production | 17,635 | 439 | 0 | Exact lease query and final publication |
| `crates/arcweft-lsp/src/session/tests.rs` | unit test | 93,783 | 2,804 | 0 | Production request-runtime fixture |
| `crates/arcweft-lsp/src/session.rs` | production | 40,788 | 994 | 0 | Synchronous legacy branch removed |
| `crates/arcweft-verify-lsp/src/lib.rs` | production | 70,155 | 1,855 | 785 | Legacy metadata resolver/API removed |

Largest non-generated production Rust files in the checkout:

| Path | Bytes | Physical LOC |
| --- | ---: | ---: |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 93,423 | 2,482 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 |
| `crates/arcweft-core/src/value.rs` | 83,366 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 |
| `crates/arcweft-bundle/src/container.rs` | 78,366 | 2,393 |
| `crates/arcweft-runtime-plan/src/expr.rs` | 84,530 | 2,382 |

Largest non-generated Rust test files are
`cli_runtime_bench.rs` (229,498 bytes/7,062 LOC),
`native_vertical.rs` (241,131/6,717),
`published_jlreq_class_mix.rs` (220,473/6,109),
`native_samples_effects.rs` (218,542/5,977),
`agent_script_debug.rs` (196,265/5,257), and sema `typecheck.rs`
(130,217/4,285).

The two changed warning-level hotspots were reviewed:

- `arcweft-lsp/src/session/tests.rs` was already 2,732 physical LOC and grows
  by 72 LOC. The new scenario belongs to the existing end-to-end session
  request suite and remains one bounded production-path test.
- `arcweft-verify-lsp/src/lib.rs` is above the production threshold and has an
  embedded test module, but this cut reduces it by 54 physical LOC and removes
  a complete obsolete responsibility. It adds no new boundary there.

No changed production Rust file grew by 300 physical LOC, no dependency or
feature boundary changed, and the audit reported no error-level issue.

## Validation

Passing final gates:

```text
cargo fmt --all -- --check
  passed

cargo test -p arcweft-lang-sema signature --lib -- --nocapture
  30 passed

cargo test -p arcweft-lsp signature --lib -- --nocapture
  5 passed

cargo test -p arcweft-lsp profiles::uri::tests --lib -- --nocapture
  2 passed

cargo test -p arcweft-lsp positions::tests --lib -- --nocapture
  7 passed

cargo test -p arcweft-verify-lsp -- --nocapture
  17 unit tests passed; 0 doc tests failed

cargo clippy -p arcweft-lang-sema -p arcweft-lsp \
  -p arcweft-verify-lsp --all-targets --all-features -- -D warnings
  passed

cargo +nightly -Zscript tools/structure-audit.rs --root .
  0 errors; 131 repository-wide warnings
```

One broader pre-existing fixture remains non-passing:

```text
cargo test -p arcweft-lsp positions --lib -- --nocapture
  7 passed; 1 failed
```

The failure is
`session::tests::entry_definition_protocol_dispatch_honors_utf8_utf16_and_utf32_positions`.
That unchanged `main` test still authors the removed manifest shape
(`[package] name = ...`, no `schema`, and an unprefixed entry ID), so the
current manifest reader rejects the profile and the definition response is
`null`. The seven position conversion tests pass when selected directly. This
cut does not restore obsolete manifest syntax or alter the unrelated fixture.

Two earlier attempts ended without a test result and were rerun successfully:
the first broad LSP signature run timed out during the shared Windows build,
and one sema rerun had its stdout pipe closed by the command timeout after
compilation. The final LSP 5/5 and sema 30/30 results above supersede those
non-results.

During fixture construction, the obsolete Rust-metadata manifest first produced
the intended typed `NoAcceptedEnvironment` failure. Replacing it with the
current production adapter manifest then exposed the extended-drive
`UriNotAccepted` mismatch; the path-owner correction and its drive/UNC behavior
tests close that production defect.

## Remaining work and deviations

Remaining work is limited to the explicitly separate cache and semantic
resource-accounting cuts. There are no design deviations in this cut. The
accepted CharacterDialogue surface is unchanged, removed `.say` and colon
surfaces remain removed, and no compatibility or source-gate mechanism was
introduced.
