# Proof concurrency v6.1.1 — public authority switch readiness and blocker audit

Date: 2026-07-21

## Audit basis and verdict

This audit was refreshed on Git
`17f222d8ab08cb27ad4c87959f9c4cfdab534ec6` / Jujutsu change
`tzqkttzrxuszmxwpzxnxwymoksttmptw`, after the private attached RichText
grammar landed. The accepted Proof contract archive remains
`D:\sanze\Downloads\arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
whose SHA-256 is
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.

The public authority switch is **blocked** at this revision. There is no
dependency-released production Rust cut that can publish the qualified
`SyntaxNodeId` or connect it to HIR without creating the prohibited detached
plus attached dual reader. Stage 5's private `HirDatabase` also cannot be
implemented ahead of the Stage 3 public syntax switch and Stage 4 final
predicate/proof surface merely because its types are fully designed.

This is a sequencing result, not a missing Proof design decision. The Proof
archive is implementation-ready and has no open questions. The unresolved
inputs below belong to existing Lang and AW-AH requests; they must not be
filled in by Proof-specific guesses.

The audit commit was subsequently rebased without conflict onto
`7ee94c30b3f64d6a40888ec885626228999e7c25`. That base adds the typed HIR View
inventory, accepted launch-profile input, and six-field dialogue profile
revision substrate. It advances Lang 01.5.1.1 but does not yet provide the
single compiler-owned `ValidatedViewProduct`, `CheckedDialogueProfile`
admission, runtime-plan materialization, or old defaults deletion required to
release the Proof switch.

## Meaning of the switch

The Proof switch is not a runtime feature flag. It is the atomic authority
change from the detached syntax and clone-based HIR paths to one
revision-bound compiler model:

```text
SourceDocument
  -> ParsedSource + SyntaxDatabaseId/SyntaxLineageId
  -> attached SyntaxNodeId family nodes
  -> arena-backed HIR/project/scope ownership
  -> sema/verifier/runtime assertion identity
  -> strict bundle/save/replay records where the identity persists
```

At final Proof closure, `TypedSyntaxTree`, line-derived identity bridges,
duplicate fragment parsers, cloned/linked HIR assembly, and source-string
reparsing are deleted. Stage 3 removes the old syntax authority; Stage 6 later
removes the old HIR/project authority. No stage publishes two public readers,
two public HIR models, a compatibility wrapper, or a detached public AST
beside `ParsedSource`.

## Current implemented prerequisite

The current private predecessor includes:

- the one-pass lossless grammar and typed event/kind inventory;
- the reduced retained global-identity inventory for Character, View, Action,
  Activity, Signal, Metric, and Layer;
- database, lineage, and attached syntax-node identities with atomic rollback;
- path-authoritative attachment for equal-kind equal-offset recovery nodes;
- a database-bound private parse product; and
- database-bound expression, type, pattern, and ordinary-statement fragment
  families using the shared lexer and grammar transaction;
- discriminant-complete private typed markers, family nodes, and role-driven
  accessors; and
- private RichText tag, ordered-argument, recovery, and expression descendants
  emitted by the same lexer/event/attachment transaction.

The direct evidence is recorded in:

- [private identity and attachment](2026-07-21-proof-concurrency-v6-1-1-stage-2-private-identity-attachment.md);
- [private typed inventory](2026-07-21-proof-concurrency-v6-1-1-stage-3-private-typed-inventory.md);
- [private attached accessors](2026-07-21-proof-concurrency-v6-1-1-stage-3-private-attached-accessors.md);
- [private bound parse product](2026-07-21-proof-concurrency-v6-1-1-stage-3-private-bound-parse-product.md);
- [private bound fragment families](2026-07-21-proof-concurrency-v6-1-1-stage-3-private-bound-fragment-families.md);
- [retained global-identity inventory](2026-07-20-proof-concurrency-v6-1-1-2-retained-global-identity-implementation.md); and
- [private attached RichText grammar](2026-07-21-aw-ah-007-008-private-rich-text-grammar.md).

These cuts intentionally expose no second public reader. They prove the target
identity and recovery behavior while production callers continue to use the
old authority until the final inventory is stable.

## Blocking dependency ledger

The public node inventory must be final before qualified syntax identity
becomes public. The checkout still has the following blockers.

| Boundary | Current checkout evidence | Required release | Existing authority |
| --- | --- | --- | --- |
| Lang 01.1.1 ordinary callable and suspension inventory | Public `FunctionKind::{Task, Dialogue, Stream}` parsing and downstream semantic/runtime branches remain. The typed await slice is only an independent predecessor. | One coherent committed migration to ordinary functions plus the selected typed suspension/role facts. Prefix/postfix propagation and project nominal resolution must use their returned contracts rather than local inference. | [Lang 01.1.1](../reviews/requests/2026-07-17-lang-01.1.1-direct-style-suspension-generator-contract-correction.md), [01.1.1.1](../reviews/requests/2026-07-20-lang-01.1.1.1-prefix-postfix-try-source-and-propagation-contract-correction.md), and [01.1.1.2](../reviews/requests/2026-07-20-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation.md) |
| Lang 01.3 Source elimination | `SourceItem`, `HirSource`, the public `Source<T, E>` family, and runtime/wire Source ownership remain. The private Proof grammar correctly gives removed source syntax no typed descendant. | Return and implement the corrected Stream definition/instance/replay/policy/AWBC/save/host contract, then delete Source through its complete atomic migration. | [Lang 01.3.1.2.1](../reviews/requests/2026-07-19-lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction.md) and [Cut 0 audit](2026-07-18-lang-01-3-1-external-stream-source-elimination-cut-0.md) |
| Lang 01.4 configured resources | The private grammar has typed `ResourceDeclarationItem`, but the public detached AST still represents configured families through generic/string-bearing `EntityDeclItem` paths and has no final public `res` owner. The generic resource/retained-reference substrate is implemented. | The complete public `res` syntax/HIR/sema consumer migration must join the authority-switch cohort. The extension-manifest decoder/encoder cannot be guessed before its wire correction returns. | [Lang 01.4.2](../reviews/requests/2026-07-20-lang-01.4.2-resource-extension-manifest-wire-contract-correction.md) and [01.4.1 audit](2026-07-20-lang-01-4-1-retained-identity-wip.md) |
| Lang 01.5 removed source owners | Public `ExternModItem`, `DialogueDefaultsItem`, and `EntityDeclKind::Content` remain even though the private grammar gives them only ordinary recovery. The single-manifest decoder plus typed dialogue-profile admission substrate have landed, but compiler catalog admission and source-owner deletion are incomplete. | Complete the single View/Style product and checked dialogue-profile authority, then commit the implementation-ready external-module and dialogue-default removal with their final consumers. Source `content` deletion must wait for typed binary content-root admission; no text overlay or directory scan may stand in for that contract. | [single-manifest audit](2026-07-20-lang-01-5-1-single-manifest-decoder-wip.md), [dialogue profile admission substrate](2026-07-21-lang-01-5-1-1-dialogue-profile-admission-substrate.md), and [Lang 01.5.1.2](../reviews/requests/2026-07-20-lang-01.5.1.2-typed-content-root-admission-contract-correction.md) |
| Dialogue-content application ownership | Private checked AW-AH-009.4.2 syntax/HIR carriers and the private RichText grammar are present, but the final public application expression, typed ID reference, coordinates, spans, and poison/execution state are not. | The AW-AH-009.4.2 public source owner must land inside the same attached syntax/HIR switch; publishing it earlier would create the second expression authority that its contract forbids. | [AW-AH-009.4.2 request](../reviews/requests/2026-07-20-aw-ah-009.4.2-dialogue-content-application-syntax-hir-ownership-production-reconciliation.md) and [private Cut 2](2026-07-20-aw-ah-009-4-2-private-cut-2.md) |
| Package-aware lowering and dialogue line identity | Lower dialogue ID newtypes are implemented and the AW-AH-009.4.3 contract has returned, but current lowering still accepts `&TypedSyntaxTree` before package/module admission and project assembly supplies package identity later. | Move every direct lower/document/project-loader/compiler/LSP caller to one checked package-aware request in an atomic compiling cut. Module-local candidates and the project collision transaction remain after the public source owner; they must not be inferred in LSP. | [AW-AH-009.4.3 intake](2026-07-21-aw-ah-009-4-3-source-site-line-identity-intake.md) |

The generated-artifact runtime-binding correction
[Lang 01.5.1.3](../reviews/requests/2026-07-20-lang-01.5.1.3-generated-artifact-runtime-binding-contract.md)
remains a later fail-closed runtime boundary. It does not authorize retaining
an obsolete source declaration, and Proof must not invent its binding key.
Likewise, `trusted axiom` remains removed; final trust continues to be metadata
on an ordinary proof.

## Required prerequisite commits

The following results must be committed and present in the public-switch base
before the switch starts. Their commit hashes are intentionally not invented;
the implementation note must record the real hashes after each result lands.

1. the already-landed private RichText prerequisite at
   `17f222d8ab08cb27ad4c87959f9c4cfdab534ec6`;
2. the complete Lang 01.1.1 ordinary-function/suspension migration, including
   every returned correction needed by its public type and propagation rules;
3. the corrected Lang 01.3 Stream runtime/wire contract followed by the atomic
   Source deletion commit;
4. the Lang 01.5 single-manifest source-owner cleanup, including typed
   content-root admission before source `content` is deleted; and
5. release of the shared syntax/HIR/sema owners used by the Lang 01.4 resource
   and AW-AH-009.4.2 public-expression slices.

Lang 01.4's public `res` migration and AW-AH-009.4.2's public expression owner
are coupled members of the switch itself, not excuses to publish partial ASTs
as separate prerequisite commits. A prerequisite is released only by a
compiling, validated commit; a dirty workspace or design archive alone is not
a release.

## Atomic public migration

Once every prerequisite above is committed, the first public cut is Proof
Stage 3. It is one workspace-compiling syntax-authority and HIR-entry change;
it is not a feature flag and cannot be split by caller family.

1. Make the incremental `ParsedSource` the sole complete-document authority,
   publishing the already-proven qualified database, lineage, snapshot, node,
   handle, diagnostic, and attached-family contracts directly.
2. Move every public syntax consumer to attached nodes, including the final
   `res` declarations, retained global-identity declarations, and the
   AW-AH-009.4.2 dialogue-content application. No public detached declaration
   or expression value remains as an alternate source owner.
3. Replace source-backed HIR entry with one checked request constructed from
   the bound `ParsedSource`, canonical package/module identity, and exact
   `SourceDocument`. Every source-backed HIR key uses the qualified grammar
   `SyntaxNodeId`; no line ID, range lookup, or reparsed substring can supply a
   HIR identity.
4. Move compiler, project-loader, LSP document, formatter, tooling, Agent REPL,
   and fragment callers together. LSP must share the syntax database and exact
   document lineage; it may not keep its own parse or identity inventory.
5. Publish the final unbound-fragment plus explicit-attachment boundary, and
   prove that an unbound fragment cannot construct the HIR lowering request.
6. Delete the old authority in the same cut rather than wrapping it.

The Stage 3 deletion set is:

- detached public `TypedSyntaxTree` ownership and `ParsedSource::typed_tree()`;
- the old public line-slot `incremental::SyntaxNodeId`, `SyntaxIdentityMap`, and
  line-derived reconciliation/semantic-parent identity authority;
- detached source-backed `Item`, expression, statement, pattern, and type
  lowering inputs once their attached replacements own those public roles;
- duplicate complete-document and source-backed fragment entry points,
  callback/body substring parsers, and raw source reparse paths;
- `lower_to_hir(&TypedSyntaxTree)` and every package-late direct lowering call;
  and
- compiler/project-loader/LSP fallbacks that reconstruct a second parsed or
  lowered source product.

The final clone/Vec HIR may remain the sole public HIR representation for the
short contract-defined interval after Stage 3, but it must accept only the new
source-backed entry and grammar identities. Adding a second public arena HIR
in that interval is forbidden.

## Required order after the syntax switch

Later Proof work remains dependency-ordered. A later stage must not be pulled
forward merely because its standalone types are already designed.

| Order | Boundary | Completion rule |
| ---: | --- | --- |
| 1 | Stage 3 attached syntax and HIR entry | The atomic cut above; one public syntax authority and grammar-node HIR keys. |
| 2 | Stage 4 predicate/proof surface | Publish the final ordinary-name predicate/proof grammar and exact `ProofBlock`; delete provisional proof strings, authored artifact IDs, old clauses, and any historical syntax carrier. |
| 3 | Stage 5 private arena HIR | Implement `HirDatabase`, immutable arenas, liveness, scopes, locals, captures, and transactions privately, after the public typed surface is final. |
| 4 | Stage 6 HIR/project public switch | Move project construction, symbols, sema, verifier, compiler, runtime-plan preparation, and semantic LSP consumers to the one arena snapshot; delete linked/cloned HIR and package-late builders. |
| 5 | Project/LSP identity consumers | Publish dialogue-line candidates and the one project collision inventory from the accepted HIR project generation; LSP only borrows that product. |
| 6 | Stage 7 runtime assertion identity | Add session-only assertion site/inventory/fault identity above core and project failures through the fresh execution diagnostic context. |
| 7 | Assertion codecs, bundle, save, and replay | Perform the one strict persisted-payload migration with no serialized syntax/HIR/session IDs and no dual reader. |

Stage 3 therefore does not change runtime assertion payloads, assertion
codecs, bundle/save/replay versions, or checkpoint semantics. The later codec
cut persists only the contract's guard/fingerprint/core assertion data; it
never persists `SyntaxNodeId`, `StmtId`, `HirSnapshotId`, a session inventory,
or a runtime fault identity.

## Current migration surface

This is a one-off `rg` inventory over `crates/**/*.rs` at
`17f222d8ab08cb27ad4c87959f9c4cfdab534ec6`, used for planning rather than as
an automated source gate.

| Old boundary | Occurrences | Rust files naming it |
| --- | ---: | ---: |
| `.typed_tree()` | 318 | 80 |
| `TypedSyntaxTree` | 49 | 20 |
| `lower_to_hir` | 638 | 65 |
| `linked_module` | 46 | 12 |
| `linked_hir` | 25 | 7 |
| `ParsedFragmentKind` | 30 | 9 |
| `SyntaxIdentityMap` | 18 | 3 |
| `SourceItem` | 18 | 8 |
| `DialogueDefaultsItem` | 19 | 9 |
| `EntityDeclItem` | 38 | 11 |

The counts include tests and may overlap. They demonstrate why a partial public
switch would create two authorities; they are not acceptance criteria and are
not checked by CI. Completion is established through typed API migration,
compile failures at removed boundaries, behavioral tests, strict codec tests,
dependency evidence, and Tier 2 execution.

## Direct-test disposition at the blocked boundary

No additional Proof-only direct test is independently useful at this revision.
The private Stage 1 through Stage 3 cuts already have direct losslessness,
typed-inventory, attachment, reconciliation, cross-database/cross-lineage,
fragment, rollback, and RichText recovery evidence in the implementation notes
linked above. The exact public syntax and compile-fail rows in the accepted
`TEST_MATRIX.md` require the sole public attached `ParsedSource` and
`LoweringRequest`; adding them now would either test the authority being
deleted or force a provisional public API. Predicate/proof tests require Stage
4, HIR/project tests require Stages 5 and 6, and runtime/codec/tooling tests
require their later ordered stages.

Accordingly, this cut adds no test-only Rust surface. The exact matrix files
and names remain required with the stage that first owns their production API;
they are not waived by this blocker result.

## Blocker-audit verification

This documentation-only cut changed no Rust source, manifest, dependency,
feature, public API, stable design chapter, codec, or fixture.

- the accepted ZIP SHA-256 matched the recorded package identity, and all 19
  non-self `MANIFEST.txt` digests matched the exact extracted member bytes;
- all 21 relative implementation and blocker-authority links in this update
  resolve in the audited checkout;
- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets
  --all-features` passed;
- `cargo fmt --all -- --check` passed;
- `git --git-dir=D:\git\arcweft\.git
  --work-tree=D:\git\arcweft-ws-proof-hir-nodeid diff --check
  17f222d8ab08cb27ad4c87959f9c4cfdab534ec6 --` passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` scanned 3,468
  files, 1,806 Rust files, 832,381 physical Rust lines, and 94 manifests; it
  reported 0 errors and 133 existing warnings and wrote no report files.

Tier 2 was not run because this cut changes only implementation-state
documentation and deliberately introduces no public/runtime/render/Agent/MCP/
capture behavior. The eventual Stage 3 and Stage 6 public migrations remain
Tier 2 integration cuts.

## Completion evidence required

The switch is complete only when:

- one accepted source revision owns every public syntax and HIR identity;
- no executable proof/assertion path can be built from detached or reparsed
  source;
- public/compiler behavior has no old-reader fallback;
- parser, HIR, project, sema, verifier, runtime assertion, bundle, save/replay,
  LSP, Agent, MCP, and formatter tests pass against the same authority;
- the applicable workspace check, strict Clippy, format check, structural audit,
  and Tier 2 suites pass; and
- the old boundaries above are deleted because their typed callers migrated,
  not because a source-spelling gate says their names disappeared.
