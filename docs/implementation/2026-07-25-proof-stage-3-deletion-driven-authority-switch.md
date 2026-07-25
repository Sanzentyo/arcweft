# Proof-concurrency v6.1.1 Stage 3 deletion-driven authority switch

Date: 2026-07-25

## Package adjudication

The implementation source is the accepted archive
[`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip),
whose SHA-256 is
`1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`.
All non-self manifest members match their declared digests. The package is
`READY_FOR_IMPLEMENTATION` for its explicitly defined boundaries. A later
field-by-field lowering audit found two narrow schema omissions that do not
invalidate the rest of the package but must not be guessed at the public HIR
boundary:

- the base and retained-global packages do not reconcile one exhaustive final
  `HirItemKind` inventory, and the retained correction names a closed
  `HirDeclarationMember` without enumerating its variants or resolving the
  alternative Character display-name representation; and
- the base package supports statement-form `if let` without a lossless
  `HirStmtKind` shape, and does not select a revision-bound owner for the exact
  unsafe-audit insertion anchor.

A subsequent independent record audit found one further public-schema gap:
the packages do not define one exhaustive final `HirExprKind` and all reachable
payloads. In addition to undefined `HirName`, `HirPath`,
`HirEntityReference`, `HirLifetimeKey`, `HirLiteral`, and ordinary
expression/pattern child records, the audit found undefined `HirCallExpr` and
`HirDialogueContent`, loss of the two placeholder meanings and thread metadata,
and the obsolete base `MemoBlock` variant. The accepted AW-AH-007/008 RichText
contract also predates the Proof arena/source-component decision: its D3.2
sketch embeds tag IDs, source spans, and nested HIR values directly, and must be
reconciled with `ExprId`, revision-bound component queries, and the final
`HirDialogueContent` without weakening its limits. These are result-changing
for literal decoding/canonicalization, call/content ownership, runtime thread
semantics, RichText diagnostics, and ID-reference precedence. Current WIP
leaf/expression records are private evidence only and are not accepted as final
authority.

The independently throwable corrections are:

- [proof 01.1.1.2.1 final HIR item/member inventory](../reviews/requests/2026-07-25-seq-proof-01.1.1.2.1-final-hir-item-member-inventory-reconciliation.md);
- [proof 01.1.1.3 statement if-let/unsafe-audit anchor](../reviews/requests/2026-07-25-seq-proof-01.1.1.3-if-let-unsafe-audit-anchor-reconciliation.md); and
- [proof 01.1.1.4 final HIR semantic leaf/expression payload](../reviews/requests/2026-07-25-seq-proof-01.1.1.4-final-hir-semantic-leaf-expression-payload-reconciliation.md).

They block only the named inventory/payload/anchor rows. `signal` target/value
and the closed trigger projection are derivable from existing typed owners and
are implemented locally rather than over-requested. All independent attached
syntax, expression/type/pattern HIR, database/project, and consumer migration
work continues. The implementation does not introduce a generic/string member,
source-less item, fabricated expression identity, payload range clone, or
syntax-AST clone while waiting.

A second direct inventory pass corrected an earlier local audit: the accepted
archive explicitly includes `SourceItem` in `LOSSLESS_TYPED_IDENTITY.md` and
`HirItem::Source(HirSourceItem)` in `HIR_DATABASE_AND_ARENAS.md`. Therefore
current `source` declarations move directly from the detached carrier to the
attached node during Stage 3. They must not be repaired in the old tree or
silently demoted to `ErrorItem`. Lang 01.3 remains the owner of their later
language/runtime removal.

The retained-global reconciliation archive remains accepted separately at
SHA-256
`0E30A91FA2F7A288E9A12D8AFC7356525604CBDC907D659CD97311207D26A68E`.
No unrecorded or changed ZIP under `docs/reviews/` was found during this
intake pass.

The immediately coupled Dialogue authority packages were reverified during
the same pass:

- AW-AH-009.4.2 SHA-256
  `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8`,
  with all 15 non-self manifest members matching;
- AW-AH-009.4.3 SHA-256
  `FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396`,
  with all 16 non-self manifest members matching.

Both report `READY_FOR_IMPLEMENTATION`, zero open questions, and require the
same deletion-driven public series: final attached application first, then
package-aware candidates/project acceptance, then deletion of every old
speaker/string/`HirDialogue` authority before workspace publication.

## Selected deletion boundary

Stage 3 is one workspace-compiling production-authority switch. Work may be
implemented by responsibility inside one local change, but no intermediate
state that publishes both readers is reviewable or pushable.

The switch deletes, rather than aliases or repairs:

- the string-only complete-document parser entry and
  `parse_document_with_source`;
- detached `source::ParsedSource`, `TypedSyntaxTree`, `typed_tree()`, and
  `into_typed_tree()` ownership;
- `FragmentKind::{Expression, Statements, Items}`, `ParsedFragmentKind`,
  `ParsedFragment`, and the detached `parse_fragment` dispatcher;
- parse-at-attachment fragment reparsing and any item or statement-list
  fragment invented outside the returned contract;
- `lower_to_hir(&TypedSyntaxTree)`, package-late direct lowering, line-derived
  source keys, and caller-local compiler/LSP reparsing; and
- source substring parsing, compatibility facades, dual readers, and source
  gates used to preserve those paths.

Compile failures after each deletion are the migration inventory. They must be
repaired toward the final owner, not by introducing a renamed text parser,
attached-to-detached projection, wrapper, alias, or fallback.

## Final owners

- `SyntaxDatabase` owns exact source lineages and publishes cheap-clone bound
  `ParsedSource` snapshots.
- `SyntaxNodeId`, `SyntaxNodeHandle`, sealed `AstNode<K>`, family nodes, spans,
  diagnostics, and Rowan round trips are owned by the attached syntax
  snapshot.
- Standalone expression, type, pattern, and one-statement parsing produces an
  `UnboundFragment<K>`. Only explicit exact-byte attachment produces an
  `AttachedFragment<K>`, and neither fragment type can construct a whole-file
  HIR lowering request.
- REPL item cells and multi-statement cells are exact synthetic whole
  documents. The contract has no `ItemFragment` or statement-list fragment.
- Compiler/project-loader share one accepted parse product per module. LSP's
  document store owns its syntax database and all feature projections borrow
  the same snapshot.
- Source-backed HIR entry accepts only a checked package/module/document key
  and the bound `ParsedSource`; every source key is the grammar
  `SyntaxNodeId`.
- Current source declarations are attached `SourceItem` nodes, matching the
  package's exact syntax and HIR inventories; no detached `SourceItem`
  projection survives.
- The base Proof archive's `DialogueCallExpression` and
  `HirExpr::DialogueCall` examples are superseded by the later accepted
  AW-AH-009.4.2 contract. The same unmerged syntax series replaces them with
  the generic postfix-bracket/colon application syntax and the final typed
  `HirDialogueContentApplication` / unresolved-postfix HIR payloads; the base
  names are not published as final owners.

## Dialogue deletion ordering

The old `SpeakerLine` / string `ContentCall` / `Expr::DialogueCall` /
`DialogueCallExpression` / `HirDialogue` path receives no repair or new
semantic identity. Stage 3's unmerged syntax series directly installs the
AW-AH-009.4.2 generic postfix/colon owner and deletes the old syntax/AST forms.
The immediately following HIR/project authority work connects that owner and
AW-AH-009.4.3 through sema and runtime-plan; the same workspace-compiling
public switch deletes the old HIR/runtime carrier and all consumers. No later
Proof runtime assertion, codec, or save/replay work is a prerequisite for that
deletion.

## Current implementation groups

1. replace the old fragment payload and parse-at-attachment predecessor with
   final unbound/attached fragment ownership;
2. publish the already-tested attached IDs, handles, typed families, exact
   accessors, diagnostics, and spans;
3. migrate Agent REPL classification and synthetic-document ownership;
4. replace the detached HIR entry and source keys;
5. migrate compiler, project-loader, CLI, tooling, and LSP to the accepted
   product; and
6. delete the remaining old source/CST/line bridge and close the public and
   compile-fail matrix before validation.

## Independent attached-expression audit

The uncommitted Stage 3 accessors received an independent contract review
before HIR expression lowering. The review found no P0 defect, but the
following rows are mandatory public-switch blockers rather than optional
cleanup:

- only the complete `Path` carries `SyntaxNodeId`; authored path segments are
  revision-bound, ID-less projections of that owner;
- `DelimitedGroup` forwards its caller's semantic role to its sole inner
  expression, including nested groups and postfix/binary parents;
- literal nodes publish the typed value classified by the canonical lexer;
  HIR does not reinterpret `raw_text()` or Rowan tokens;
- compact numeric sequences retain one expression identity plus ID-less typed
  values and ranges, and an unterminated trailing separator still owns its
  `MissingExpression` recovery boundary;
- `AwaitExpression` retains `applies_try`, and `CallArgument` retains its
  positional, named, or spread form;
- the current `=>` implication operator lowers through the owned `BinaryOp`
  vocabulary without colliding with match-arm delimiters;
- missing record-field names use `MissingName`, missing closure punctuation
  uses `MissingTokenNode`, and a `match` without its required opening brace is
  incomplete/recovered rather than a clean empty match; and
- Agent REPL classification observes those same fragment recovery facts while
  item and multi-statement cells remain synthetic whole documents.

Focused evidence must cover grouped-expression roles, all literal families,
plain and propagating await, all three call-argument forms, compact numeric
exact/trailing/unterminated cases, implication versus match-arm context, and
the named recovery cases above. The old `lower_to_hir` and
`lower_document_to_hir` references currently preventing broader Agent/compiler
tests are deletion inventory; they are migrated to the final lowering request,
not restored to make this intermediate checkout green.

This note records an in-progress atomic change. Completion requires focused
syntax/HIR/consumer tests, workspace check and strict Clippy, normal workspace
tests, Tier 2, structural audit, and a coherent push to `main`.

## Repository policy

The current `AGENTS.md` already requires root-cause deletion of obsolete
variants/functions/types and forbids unfinished parser/compiler compatibility
aliases, wrappers, migration shims, dual readers, and source gates. No policy
edit is required for this switch.
