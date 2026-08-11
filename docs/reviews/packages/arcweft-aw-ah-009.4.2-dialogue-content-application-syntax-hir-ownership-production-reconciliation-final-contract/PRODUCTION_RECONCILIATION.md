# Production reconciliation

## 1. Inspected baseline

```text
repository: Sanzentyo/arcweft
branch: main
commit: e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
Jujutsu change: UNAVAILABLE_FROM_REMOTE_GIT_SNAPSHOT
inspection mode: private GitHub connector, exact refs, read-only
```

The baseline was rechecked after `main` advanced during inspection. The final
head commit only consolidates an unrelated Stream source-elimination review and
does not alter the relevant parser, syntax AST, call surface, HIR blobs, or the
canonical `CharacterDialogue` authoring documentation in its parent commit.
The contract targets that final head rather than the earlier observed commit.

## 2. Predecessor identities

```text
AW-AH-009.4
sha256 a86044fea7aaff3ec3829dfa0ad6552c88377ca61fa2911c3b96ea34ca0ffa5e

AW-AH-009.3.1
sha256 6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588

proof-concurrency v6.1.1
sha256 1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef

AW-AH-009.3.2
sha256 8701ff3ae6024cd62c33c4b36abdfa358bfa30aa93209655870c475eea1dd40d
```

The exact identities and indexed package evidence were available. The four ZIP
byte streams were not mounted into this artifact runtime, so this package does
not claim a fresh byte-for-byte re-extraction of those predecessor archives.
That evidence boundary does not leave a design choice open.

## 3. Preserve matrix

| Production owner | Inspected evidence | Final action |
|---|---|---|
| `crates/arcweft-lang-syntax/src/expr/call_syntax.rs` | blob `cf3b3663a44b64ca559d921212b7468fbd0fe351`; private fields, checked parser constructors, parenthesized/callback surfaces, exact argument ranges | Preserve as the sole ordinary-call source contract. Extend only the ordinary expression value carrier from `EntityRefSyntax` to existing `IdRef`. |
| `docs/implementation/2026-07-20-aw-ah-009-3-1-call-surface-production.md` | blob `b89d69...`; landed direct call substrate and no dialogue adapter | Preserve. Do not implement superseded `SpeakerLineSurface`/`ContentCallSurface` handoff. |
| `crates/arcweft-dialogue` and AW-AH-009.4 Cut 1 | runtime/domain substrate already implemented and validated | Preserve without semantic, schema, limit, patch, role, registration, alias, or wire redesign. |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | blob `95af293...`; existing `DialogueContent`, RichText, interpolation, controls, and source ranges coexist with old speaker/string nodes | Preserve the content model; delete old speaker/string application ownership. |
| `crates/arcweft-lang-syntax/src/ast/line_plan.rs` | blob `af5135a88c3410758b1f0e2a72706581f4ff2ed0`; canonical `LinePlan` and `LinePlanItem` | Preserve syntax and behavior. Add only application attachment ownership and typed-HIR child IDs. |
| proof-concurrency v6.1.1 | source-backed IDs, `HirSlotMetadata`, typed arenas, scopes, transactions, immutable snapshots, recovered tooling state | Preserve and extend the original enums/inherent implementations. No parallel arena. |
| AW-AH-009.3.2 | one accepted `Arc<HirProject>`, no on-demand LSP parse/lower | Preserve exactly. |
| `crates/arcweft-lang-syntax/src/ast/ids.rs` | blob `3964e11f4b98354f9f2fbfa284d6445c2e9d297e`; existing `IdRef` already has absolute, relative, family-relative forms | Preserve `IdRef`; make it the ordinary expression carrier instead of inventing another ID type. |

## 4. Concrete defects requiring replacement

### 4.1 Name/shape heuristic in the shadow parser

`crates/arcweft-lang-syntax/src/parser/expression.rs`, blob
`8fcd3cf4ddfeca98e4937973d645bb77f2ebdfe5`, currently has a special Flow
entry that selects `DialogueCallExpression` from whether it saw a call, a
non-ASCII first character, or `TextToken`. Ordinary Pratt parsing emits
`IndexExpression` directly. This is not a retained generic postfix substrate
and cannot preserve exact ambiguity.

**Replacement:** one generic postfix CST and two bounded typed candidates.

### 4.2 String reconstruction and delimiter search

`crates/arcweft-lang-syntax/src/parser/dialogue.rs`, blob
`e4bd5dce...`, and related helpers reconstruct dialogue call information and
search source/delimiters after parsing. This conflicts with typed-source and
source-map authority.

**Replacement:** parser-owned surfaces and direct typed lowering; delete the
search helpers.

### 4.3 Old syntax application owner

`Expr::DialogueCall`, `DialogueCallExpression`, `SpeakerLine`,
`SpeakerLineSurface`, and string `ContentCall` encode the removed speaker/call
model rather than canonical `CharacterDialogue` application.

**Replacement:** `Expr::DialogueContentApplication` plus bounded
`Expr::PostfixBracket` ambiguity.

### 4.4 Flow-only HIR sidecar and syntax clones

`crates/arcweft-lang-hir/src/model.rs`, blob
`fb4aad1ba6932fc914b3fc755d65a47f4e992a9a`, currently stores
`HirFlowItem::Dialogue(Box<HirDialogue>)`; `HirDialogue` retains speaker
surface, strings, syntax expressions, content, and `LinePlan`. Functions still
retain syntax `Stmt`/`AuthoredExpr` fields. `lower_dialogue.rs`, blob
`d52927d49b74b8489f4e67ed253419e04512e2de`, feeds that sidecar.

**Replacement:** the accepted proof-HIR database/arena public switch, with
application and ambiguity as `HirExprKind` variants usable in every body.

### 4.5 Missing relative-ID expression carrier

`IdRef` supports all required forms, but `Expr::EntityRef` currently carries
`EntityRefSyntax`, whose variants omit unqualified `Relative`. Re-reading
argument source later would violate the accepted source contract.

**Replacement:** make `Expr::EntityRef(IdRef)` directly and lower to
`HirExprKind::EntityReference(HirIdRef)`.

## 5. Current-main documentation alignment

`docs/01-language/character-dialogue.md` at the final head states that
`CharacterDialogue` is the sole configured dialogue value, bracket and colon
forms directly apply content, plans attach to that application, and the old
speaker/`.say` surfaces have no compatibility parser. This contract supplies
the production-ready CST/AST/HIR details that the documentation intentionally
does not define.

## 6. Non-redesign findings

No inspected evidence requires changing:

- Character registration, alias resolution, callable catalog precedence, or
  Cut 1 runtime value semantics;
- ordinary call groups, callback blocks, argument ranges, signature-help
  resolver policy, or missing-`)` behavior;
- source document identity, revision identity, accepted project publication,
  scope/liveness rules, or HIR transaction semantics;
- `DialogueContent`, RichText, controls, interpolation semantics, or line-plan
  semantics;
- runtime wire records, View projection, voice/TTS policy, or text rendering.

Those areas are therefore explicitly outside the implementation diff except
for mechanical exhaustive-match migration required by the public replacement.

## 7. Dependency boundary

The final dependency direction remains:

```text
syntax -> HIR -> sema -> runtime-plan / verify -> tooling
```

Syntax gains no HIR/sema/runtime dependency. HIR gains no runtime-host/core
back-edge and no production Serde dependency. Tooling consumes the accepted
project snapshot; it does not construct one.
