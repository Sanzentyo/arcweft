# AW-AH-009.4.3 source-site line identity intake

Date: 2026-07-21

## Package evidence

The authoritative package is:

```text
D:\sanze\Downloads\arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip
```

Its SHA-256 is
`fd9f97d37b857991120dd5e5e5db27953257121fc48c79beef4fa03df1f23396`.
The archive contains 17 unique entries and 107,038 uncompressed bytes. Every
entry was read to completion, every non-self `MANIFEST.txt` length and SHA-256
matched, and the manifest self row uses the documented all-zero digest.
`OPEN_QUESTIONS.md` contains exactly `none`, and the package reports
`STATUS=READY_FOR_IMPLEMENTATION`.

The package audited repository baseline
`27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9`. That revision is an ancestor of
the intake baseline `0fa18e252fe3`. The package contains no production patch;
its 100-row `TEST_MATRIX.md` and eight-frontier implementation handoff are the
acceptance authority.

The originating repository request is:

```text
docs/reviews/requests/2026-07-20-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation.md
```

The returned contract resolves that request. No duplicate AW-AH-009.4.3 design
request is required.

## Selected final contract

The package fixes these ownership boundaries:

- `arcweft-id::dialogue` owns checked `DialogueLineId(PublicId)`,
  `DialogueTextKey(TextKey)`, and `MAX_DIALOGUE_ID_BYTES = 256`;
- package, canonical module path, and exact source provenance enter HIR before
  allocation through one checked `HirModuleKey` and `LoweringRequest`;
- each source-backed dialogue application produces at most one bounded,
  module-local, unaccepted candidate;
- line IDs are derived from the flow or callable source owner and named lexical
  scopes, never from Character, speaker, callee spelling, aliases, or display
  names;
- generated ordinals are deterministic per final prefix, start at one, use a
  minimum width of three digits, and never skip an occupied candidate;
- explicit or failed sites do not consume generated ordinals;
- explicit text keys must be absolute; an absent key derives
  `text.<line-id-body>` and text keys are not a uniqueness namespace;
- exactly one package-qualified `HirProjectBuilder` performs the project-wide
  collision transaction and publishes one immutable accepted inventory inside
  `HirProject`;
- AW-CD-013 and AW-CD-020 through AW-CD-028 use the existing
  `arcweft_source::Diagnostic` transport with revision-bound primary and
  secondary labels; and
- all sema, runtime-plan, verifier, LSP, Agent, MCP, CLI, reference, and rename
  consumers borrow the same accepted `Arc<HirProject>` generation.

The package explicitly forbids speaker-derived identity, late package
qualification, a second LSP inventory, probing for free generated IDs,
compatibility wrappers, dual readers, old public builders, and repository
source-spelling gates.

## Production reconciliation

At intake, production still has the provisional model:

| Contract owner | Current production evidence | Required direct replacement |
|---|---|---|
| Package-aware lowering | `lower_to_hir(&TypedSyntaxTree)` lowers before package and module-source admission. | One checked `LoweringRequest` binds package, canonical module, incremental source snapshot, and exact document before any HIR allocation. |
| Project identity | `HirProject::new(package, modules)` introduces package identity during project assembly and indexes modules by `CanonicalModulePath`. | Package-qualified `HirModuleKey` snapshots and one transactional `HirProjectBuilder`. |
| Line identity | `DialogueSpeakerSlug`, flow/speaker/scope strings, and mutable line counters participate in generated IDs. | Typed source-owner prefixes and bounded module-local candidates with no Character or callee input. |
| Lower diagnostics | `HirLowerError` retains only a message and optional `TextRange`. | Structured fatal/candidate/project diagnostic kinds projected through `arcweft_source::Diagnostic` and exact `SourceSpan` labels. |
| Accepted line product | `HirProject` has no project-wide accepted dialogue-line inventory or source-`ExprId` lookup. | One immutable `AcceptedDialogueLineInventory` owned by the accepted `HirProject`. |
| Source application owner | Only the private AW-AH-009.4.2 carrier substrate has landed. | The final public source-backed application CST/AST/HIR arena must land before candidate production. |

The existing private AW-AH-009.4.2 work is documented in
`docs/implementation/2026-07-20-aw-ah-009-4-2-private-cut-2.md`. It is not the
public expression owner required by AW-AH-009.4.3 Frontier 1, so successful
line-candidate production cannot begin yet.

## Parsed-source interpretation

The package's `LoweringRequest<'a>` example names `ParsedSource` while the
current syntax crate has both `arcweft_lang_syntax::source::ParsedSource` and
`arcweft_lang_syntax::incremental::database::ParsedSource`.

This does not require another design request. The package's precedence clause
retains proof-concurrency v6.1.1, whose request explicitly selects an attached
incremental `ParsedSource`. Therefore:

- `LoweringRequest::syntax` means the incremental database `ParsedSource`;
- `SourceSnapshotId` is borrowed from that value's checked `snapshot()`
  accessor rather than copied into another independently supplied field;
- its attached parsed document is compared with the separately supplied
  `SourceDocument` and `HirModuleKey::source()` before allocation; and
- the project loader supplies the canonical package/module mapping used by the
  checked request constructor. Lowering does not reconstruct loader topology.

This is an implementation reconciliation of named existing owners, not a new
public contract or compatibility layer.

## Compiling frontiers and current readiness

The package's eight frontiers remain the implementation order.

1. **AW-AH-009.4.2 public source owner — blocked by predecessor
   implementation.** The final application expression arena, immediate
   coordinates, typed `HirIdRef`, component spans, and executable/poison state
   are not yet public production facts.
2. **Lower durable/private substrate — designed and independently
   mergeable.** The lower ID newtypes and constant can land first. The
   CharacterDialogue limit must then source its unchanged value from the lower
   constant. Private HIR owner/candidate/diagnostic/transaction shapes may land
   without exposing a successful line path.
3. **Package-aware lowering — designed but atomic.** Every direct
   `lower_to_hir`, document, project-loader, compiler, and LSP caller must move
   together; the old package-late entry point is deleted, not wrapped.
4. **Module-local candidates — sequencing-blocked by Frontier 1.**
5. **Project transaction — private preparation is allowed; successful public
   project construction must not coexist with the old builder.**
6. **Atomic public replacement — sequencing-blocked by Frontiers 1, 3, 4, and
   5.** All accepted consumers move to the same generation in one compiling
   series.
7. **Deletion proof — follows the replacement.** Compile-fail and behavioral
   tests replace source scans.
8. **Broad and Tier 2 validation — required before completion.** All 100
   package test rows, strict Clippy, normal workspace validation, structural
   audit, and `just test-tier2` must pass against current IDs and authored View
   geometry.

This intake is therefore not a completion claim. Frontier 2 is the only
implementation-ready independent slice at the current production boundary.

## Shared-working-copy conflicts

The intake occurred while other accepted cuts owned the files needed by
Frontier 2 and Frontier 3. Those changes must be committed or otherwise
released before this package edits them.

| Shared owner | Conflicting paths | AW-AH-009.4.3 need |
|---|---|---|
| AW-AH-009.4.1.1 Character/localization cut | `crates/arcweft-id/Cargo.toml`, `crates/arcweft-id/src/lib.rs` | Publish the lower `dialogue` module and its tests without mixing locale work into this cut. |
| AW-AH-009.4.1.1 Character/dialogue cut | `crates/arcweft-dialogue/src/character_dialogue/identity.rs` | Source the existing 256-byte field value from `MAX_DIALOGUE_ID_BYTES` without changing behavior. |
| Proof/resource HIR cut | `crates/arcweft-lang-hir/src/lib.rs`, `lower.rs`, `model.rs`, `cache_facts.rs` | Add module/request identities, structured lower errors, and private line substrate without overwriting the active HIR refactor. |
| Typed-resource syntax/HIR cut | `crates/arcweft-lang-syntax/**`, `crates/arcweft-lang-sema/**` | Frontier 3 and later caller migration must start from the accepted post-cut tree. |

No Rust file in these sets was edited during intake. A new
`crates/arcweft-id/src/dialogue.rs` would still require the currently owned
`lib.rs` and Cargo/test integration, so it was not created as an orphaned
partial slice.

## Frontier 2 lower-ID sub-slice

The Character/localization Cut 1 released `arcweft-id` at
`b08d87a3e118e1b72160b536505359d4a2d4e282`. Starting from the subsequent
intake commit `acd05f6c5c8d4be81ef050579dc02f320f8999d5`, the first Frontier 2
sub-slice now adds:

- public `arcweft_id::dialogue` ownership without a root compatibility
  re-export;
- checked `DialogueLineId(PublicId)` and `DialogueTextKey(TextKey)` newtypes;
- one shared `MAX_DIALOGUE_ID_BYTES = 256` lower-layer constant;
- exact `say.*` and `text.*` family/nonempty-tail validation after the existing
  base-type validation;
- inclusive UTF-8 byte validation with a typed `TooManyBytes` error;
- read-only base-type and string access, owned extraction, `FromStr`, and
  checked owned-base conversions; and
- no `Serialize` or `Deserialize` implementation.

This completes package matrix rows TM-001 through TM-005 and TM-007. TM-006,
which sources the unchanged CharacterDialogue field from the lower constant,
remains outside this sub-slice because the CharacterDialogue identity file is
still owned by the active Character/localization reconciliation. The private
HIR owner/candidate/diagnostic/transaction portion of Frontier 2 likewise waits
for the active proof/resource HIR cut.

The compile-fail cases directly prove that neither tuple field is externally
constructible and that neither durable identity implements Serde. They are
public API type checks, not repository source gates.

### Validation

The following commands passed against parent revision
`acd05f6c5c8d4be81ef050579dc02f320f8999d5` and the isolated ID changes:

```bash
cargo fmt -p arcweft-id
cargo test -p arcweft-id --all-features
cargo clippy -p arcweft-id --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The focused suite passed 16 unit tests and one trybuild harness containing
three compile-fail cases. Strict Clippy produced no warnings. The structural
audit scanned 3,411 files, 1,773 Rust files, and 814,836 Rust physical lines;
it reported zero errors and 131 pre-existing size/ownership warnings. No
changed `arcweft-id` file reaches a warning threshold.

### Structural measurements

The owning crate has 21 normal workspace dependents and two normal outbound
dependencies (`serde` for the already accepted locale owner and `thiserror`).
The dialogue module itself adds no higher-layer dependency.

| Path | Role | Bytes | Physical LOC | Embedded test LOC | Responsibility |
|---|---:|---:|---:|---:|---|
| `crates/arcweft-id/src/lib.rs` | facade | 10,476 | 374 | 67 | Deliberate module publication plus existing lower identity facade. |
| `crates/arcweft-id/src/dialogue.rs` | production + unit tests | 8,734 | 296 | 84 | Durable dialogue newtypes, checked family/byte rules, access and conversion APIs. |
| `crates/arcweft-id/tests/public_api.rs` | integration compile-fail harness | 313 | 7 | n/a | Private-field and non-Serde API proof. |
| `crates/arcweft-id/tests/ui/dialogue_identity_private.rs` | compile-fail fixture | 184 | 6 | n/a | `DialogueLineId` raw constructor rejection. |
| `crates/arcweft-id/tests/ui/dialogue_text_key_private.rs` | compile-fail fixture | 191 | 6 | n/a | `DialogueTextKey` raw constructor rejection. |
| `crates/arcweft-id/tests/ui/dialogue_identity_not_serde.rs` | compile-fail fixture | 390 | 12 | n/a | Both durable identities reject Serde bounds. |

The largest current workspace Rust files remain unrelated: generated Unicode
vertical-orientation data (12,399 LOC), CLI runtime-bench integration tests
(7,062 LOC), and ignored Tier 2 native Agent-observe tests (6,717 LOC). This
slice neither changes nor depends on those ownership hotspots.

## Existing requests and non-goals

The following existing requests already own the relevant predecessor and
adjacent decisions:

- `docs/reviews/requests/2026-07-20-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation.md`;
- `docs/reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md`;
- `docs/reviews/requests/2026-07-16-aw-ah-009.3.2-accepted-hir-request-lifecycle-production-reconciliation.md`; and
- `docs/reviews/requests/2026-07-20-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation.md`.

No new follow-up request was created. The remaining blockers are implementation
sequencing and shared-file ownership, not an unresolved result-changing design
decision.

This intake does not:

- implement a provisional parser, AST, HIR, or successful line path;
- recreate AW-AH-009.4.2 coordinates by reparsing source;
- change CharacterDialogue limits or runtime behavior;
- preserve speaker-derived IDs, old errors, old project builders, or old
  lowering through aliases or wrappers;
- add a source gate or removed-syntax recognizer; or
- count the package as complete before its 100-row matrix and Tier 2 closure.

## Next implementation action

After the CharacterDialogue and HIR owners publish their coherent cuts,
complete the remaining Frontier 2 work:

1. source the unchanged CharacterDialogue byte limit from the lower constant;
2. add private HIR owner, scope, candidate, diagnostic, and transaction
   substrate with no public successful line path;
3. run focused changed-crate tests and strict Clippy;
4. rerun the structural audit and update exact metrics; and
5. commit and push that slice before beginning the atomic package-aware
   lowering migration.
