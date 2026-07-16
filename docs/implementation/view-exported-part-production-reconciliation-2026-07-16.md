# View exported-part production reconciliation

- Date: 2026-07-16
- Package: `arcweft-seq-06.11d.2.1.1.1-view-exported-part-authoring-production-reconciliation-final-contract.zip`
- Package SHA-256: `5e00a432a1d957a85d9706a6eea7f3f5d67a380c4c8c443cc8b403693cd617ac`
- Package basis: Git `76d39983ad8770a87d6e81745785b6b362a381b4`
- Rebased production base: Jujutsu change `rxzqqylv` / Git `b266271774f3`
- Implementation stack at review: parent change `omuxqqlv` / working change `zrvqpuvs`
- Status: safe production reconciliation implemented; complete package acceptance remains design-gated

## Package intake

All twelve package members were read. Every ordinary manifest digest matches,
and the `MANIFEST.txt` self-entry matches the documented 64-zero SHA-256 rule.
The archive says `READY_FOR_IMPLEMENTATION`; the production checkout nevertheless
reproduces the result-changing contradictions recorded below.

## Implemented reconciliation

This stack retains the earlier canonical authoring implementation and adds:

- revision-bound `SourceDocumentIdentity`/`SourceSpan` ownership in
  `ParsedSource`, View-part syntax records, HIR owners/targets/exports, checked
  records, compiler projection, formatter metadata, and LSP metadata;
- distinct checked `ViewPartLocalName` and `ViewPartName` values using the
  closed dotted-name grammar, exact byte/segment limits, private fields, and
  checked compact part/instruction IDs;
- one fallible candidate-first `ViewProgramBuilder` static-part inventory with
  canonical local/export ordering, total node-producing instruction kinds,
  reachability/site evidence, duplicate checks, `CallView` export rejection,
  and complete finish-time validation;
- source-aware checked export projection that rejects an identity, revision,
  length, or range mismatch instead of accepting an ambient source string;
- typed product name/owner provenance validation and runtime/LSP adaptations
  for the current single-source product boundary;
- a corrected CLI authored pipeline test in which one registered
  `SourceDocument("test.arcw")` and its exact bytes flow through parse, HIR,
  checking, compiler projection, and product source refs; and
- deletion of the spelling-specific `exportparts` and `.export_part(...)`
  recognizers/tests. Removed forms now follow ordinary current-grammar recovery
  and no historical part node is retained.

The CLI correction is important production evidence: the previous test helper
created a one-megabyte blank source context after parsing a different in-memory
document. The new checked projection correctly rejected that mismatch. The
test now supplies the same registered identity and bytes at both boundaries.

## Result-changing contract contradictions

1. The required `ViewId(PublicId)` cannot directly replace the current numeric
   `ViewId` because that type is also the public dense key for anonymous
   host-registered Rust Views, entities, fragments, and `ViewRegistry`.
2. The required bundle API names `arcweft_source::SourceDocument`, but
   `arcweft-bundle` has no `arcweft-source` dependency while the same contract
   forbids every dependency addition except `arcweft-view -> blake3`.
3. Permanent spelling-specific removed-syntax diagnostics/tests required by
   the package conflict with the repository-wide final-removal rule in
   `AGENTS.md` and the user's explicit direction.

These decisions control the final multi-source SourceMap, typed instruction
target table, owner/program catalog, private occurrence identity, accepted
replacement transaction, targeted invalidation, and contextual tooling. A
local compiling choice would select public identity and dependency behavior not
authorized by either contract.

The independent correction request is:

- [seq-06.11d.2.1.1.1.1 production contract correction](../reviews/requests/2026-07-16-seq-06.11d.2.1.1.1.1-view-exported-part-production-contract-correction.md)

## Explicit remaining package requirements

The following are not claimed complete:

- canonical multi-source `ProductSourceId`/binary `SourceMapSection` and
  complete-product source-index typestate;
- final `ViewDefinitionRef`/`ViewId`/`ViewProgramId` and immutable
  `ViewProgramCatalog` authority;
- typed product instruction targets/static inventory/fingerprint transcript;
- semantic-site occurrence reconciliation and opaque direct-boundary
  capability replacing every boolean/string fact;
- accepted revision/mount generation, six-phase replacement, exact rollback,
  and targeted cache/trace/tooling invalidation;
- complete application-edge contextual Style binding and atomic LSP rename,
  symbols, and semantic-token lifecycle; and
- the original matrix's broad SM/CODEC/ID/RT/HOT/INV/LSP/MIG and exact-limit
  suites after their owning contracts are corrected.

## Verification evidence

Commands use `CARGO_INCREMENTAL=0` and the stable feature combination shown.

- affected-crate `cargo check ... --all-targets` — passed;
- `cargo test -p arcweft-view view_program` — 2 passed;
- `cargo test -p arcweft-view --test style_metadata` — 10 passed after
  retaining the rebased physical-geometry metadata alongside part identity;
- `cargo test -p arcweft-lang-syntax --test view_export_part` — 6 passed;
- `cargo test -p arcweft-lang-sema view_part` — 4 direct tests passed;
- `cargo test -p arcweft-bundle --test view_resource_codecs exported_part` — 6 passed;
- `cargo test -p arcweft-cli authored_export_part_lowers_to_typed_product_inventory` — 1 passed after the source-identity fix;
- `cargo test -p arcweft-runtime-driver --test view_runtime exported_part` — 1 passed;
- `cargo test -p arcweft-tooling --test view_export_part` — 2 passed;
- `cargo test -p arcweft-lsp view_part` — 2 direct tests passed; and
- changed-crate `cargo clippy ... --all-targets --all-features -- -D warnings` — passed across the 13 affected crates.
- `cargo fmt --all -- --check` — passed;
- added-line patch whitespace/conflict scan over `jj diff --git` — passed;
- removed-spelling search over syntax/CLI code — no `exportparts` or
  `.export_part(...)` recognizer remains; and
- added source-gate construct audit over the patch — no `include_str!` or
  `read_to_string` was added; production `.contains(...)` additions are
  value/name validation or typed collection membership, while test additions
  assert formatter/diagnostic output. None reads checked-in source.

A passing narrow suite is not represented as proof of the unimplemented package
groups above.

## Structural audit

The canonical audit ran at Jujutsu change `zrvqpuvs`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

It scanned 2,904 files, 1,439 Rust files, 671,662 Rust physical LOC, and 90
package manifests. The result was 0 errors and 129 repository-wide warnings.
Reports were also generated under the ignored `target/structure-audit-view-export`
directory for exact review.

The new responsibility modules stay below production warning thresholds:

| Path | Bytes | Physical LOC | Role | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-view/src/part.rs` | 14,601 | 473 | name, identity, and part-boundary types | 36 |
| `crates/arcweft-view/src/program.rs` | 32,761 | 965 | fallible program builder and validation | 230 |
| `crates/arcweft-lang-hir/src/lower.rs` | 15,040 | 447 | declaration/HIR lowering | 177 |

Eight changed legacy files remain over warning thresholds: bundle View model
(78,043 bytes/2,393 LOC, including 42 embedded test LOC), bundle View codec
(86,155/2,364), CLI bundle orchestration (71,777/2,018), its test module
(81,534/2,545), semantic module checking (90,045/2,367), View AST
(49,676/1,810), parser item dispatch (55,167/1,574), and View parsing
(58,704/1,736). This slice only adapts their existing boundary call sites; it
does not add another responsibility to them. The substantive new validation is
owned by `arcweft-view::part` and `arcweft-view::program`, so splitting those
legacy files as part of this package would mix an unrelated structural refactor
into the production contract reconciliation.
