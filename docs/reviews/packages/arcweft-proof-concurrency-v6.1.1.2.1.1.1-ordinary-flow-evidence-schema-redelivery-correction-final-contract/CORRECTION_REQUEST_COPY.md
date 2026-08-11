# Design request: Proof 01.1.1.2.1.1.1 ordinary Flow evidence and schema corrected redelivery

- Date: 2026-08-02
- Sequence: Proof-concurrency v6.1.1.2.1.1.1, correcting the rejected
  v6.1.1.2.1.1 return before the ordinary Flow/Thread final-HIR authority
  switch
- Kind: independently throwable, GitHub-only, design-only full replacement
- Production changes: prohibited
- Output language: English
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip`

## Assignment

Return one standalone, decision-complete replacement contract for ordinary
source-level `Flow`, its attached syntax, final HIR, revision-bound source
index, and the `HirThreadBody` projection shared with `ThreadExpression`.

Use the latest GitHub `main` from:

```text
https://github.com/Sanzentyo/arcweft
```

Read the repository `AGENTS.md`, the applicable Rust skill, this correction,
the complete primary request, its parent request, every predecessor and
implementation/intake note named by those requests, every accepted Proof and
Lang-01.1.1 package selected by those notes, the maintained language chapters,
and all current syntax/HIR/sema/compiler/runtime-plan/LSP/Agent consumers.

This is a design-only assignment. Do not edit production code, tests,
manifests, stable design chapters, or fixtures. Do not create a branch, patch,
PR, implementation overlay, migration crate, compatibility layer, or source
gate.

## Rejected return

The prior archive was inspected externally at:

```text
D:\sanze\Downloads\arcweft-proof-concurrency-v6.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation-final-contract.zip
```

Its identity is:

- byte length: `21,466`;
- SHA-256:
  `F4F18E08B7D5A561B352D8B344734F7E892B290EC6D276DECF2A90F4F4D4FF3E`;
- 15 members; all 14 listed member hashes are exact;
- self-status `READY_FOR_IMPLEMENTATION`; and
- `OPEN_QUESTIONS.md` equal to `none` plus its final newline.

The repository rejects that READY claim. See:

- `docs/implementation/2026-08-02-proof-ordinary-flow-return-intake.md`

The rejected archive is not an accepted predecessor and supplies no final
Flow schema. It says the repository, remote, baseline revision, `AGENTS.md`,
all predecessor documents, and all production consumers were unavailable. It
then incorporates the primary request by reference and supplies generic owner
rules instead of answering the request's concrete questions.

Do not rename, wrap, or resubmit that archive. Do not return a delta that
depends on it. Build a complete replacement from current GitHub evidence.

## Required repository reconciliation

At minimum, read in full:

1. `AGENTS.md`;
2. `docs/reviews/requests/2026-07-31-seq-proof-01.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation.md`;
3. `docs/reviews/requests/2026-07-25-seq-proof-01.1.1.2.1-final-hir-item-member-inventory-reconciliation.md`;
4. `docs/implementation/2026-08-02-proof-ordinary-flow-return-intake.md`;
5. `docs/implementation/2026-07-26-proof-hir-local-schema-decisions.md`;
6. `docs/implementation/2026-07-17-proof-concurrency-v6-1-1-stage-1-flow-header.md`;
7. the latest final-HIR scope, source-owner, tail-owner, Call, Select,
   Dialogue, Predicate, Proof, Function, item/member, project, and public-switch
   notes linked by those documents;
8. every accepted Proof-concurrency and Lang-01.1.1 ZIP selected by those
   notes;
9. `docs/01-language/grammar.md`, `contracts.md`, `block-scopes.md`,
   `syntax.md`, and the maintained Flow/dialogue/control-transfer chapters;
   and
10. all current Flow and Thread/Flow-item parser, attached-syntax, final-HIR,
    source-index, sema, compiler, runtime-plan, formatter, LSP, CLI, Agent, and
    project-publication readers.

Record the exact inspected Git commit, dirty/clean state, hashes of every
normative repository input, and file/line evidence for every current consumer.
If GitHub or repository access is unavailable, the package cannot claim
`READY_FOR_IMPLEMENTATION`.

## Corrections that must be concrete

The primary request remains normative in full. The replacement must answer it,
not restate or merely “adopt” it.

### 1. Exact attached-syntax owner

Define compilable Rust-level records/enums, fields, visibility, constructors,
and read-only accessors for the Flow item, common prefix/header, all four
identity states, generics, the sole optional parameter group, return
annotation, `where`, clauses, statement-only body, unclosed/missing recovery,
and trailing recovery.

For every field, define the exact parser projection, `SyntaxKind`,
`SyntaxRole`, source component, ordinal, range/insertion owner, and recovery
state. State exactly which identity supplies semantic callable identity and
which supplies an optional presentation name. Do not fabricate an ID or name.

### 2. Exact final-HIR schema and signature semantics

Define the exact `HirItemKind::Flow` payload and every referenced type. Close:

- authored versus omitted return storage;
- semantic `Unit` without a fake `TypeId` or source node;
- callable scope, parameter locals, body scope, contract scopes, and parents;
- `ensures` result-local existence, type, scope, source origin, visibility,
  generation, and poison behavior;
- zero parameters, one parameter group, rejected second group, missing
  parameter/return type, malformed `where`, and unclosed/missing body; and
- deterministic arena allocation and source-order publication.

### 3. Complete contract-clause table

Give one normative row for each of:

```text
requires, ensures, invariant, assume, reads, effects, no_effect,
modifies, decreases
```

Each row must select admitted/non-applicable status, exact attached variant,
exact final-HIR payload, child ID kinds, path/proof/effect/callable authority,
scope and result visibility, interleaving order, duplicate behavior, missing
payload recovery, source roles, primary/related diagnostics, poison
precedence, and exact `ContractClauses` accounting.

### 4. Statement-only body and exhaustive Flow-item table

Define the exact Flow-specific and Thread-specific attached body owners. Neither
may expose an ordinary value-block tail.

Give one normative row for every accepted `HirThreadFlowItem` variant:

```text
Statement, DialogueApplication, Choice, If, IfLet, Match, Loop, While,
WhileLet, For, Select, SourceLocale, Scope, Include, AwaitWith, Error
```

Each row must close the admitted attached kind and contextual grammar, exact
`StmtId`/`ExprId` payload, body and nested scope owner/parent, local visibility,
source role/ordinal, recovery child, primary issue, empty/missing/malformed
behavior, Flow-versus-Thread admission, allocation order, and source-freeze
checks for substitution, duplication, reordering, stale/foreign IDs, and
rollback.

If maintained current grammar proves that a listed variant has no authored
surface, classify or delete it explicitly. Do not invent syntax or route a
typed Flow item through generic `Statement` merely for convenience.

### 5. Poison, accounting, limits, and transaction table

Define one primary-issue precedence across prefix, identity, signature,
clauses, body, body children, unclosed body, and trailing recovery. Give exact
preflight charges and inclusive exact/one-over outcomes for items, identities,
generics, parameters, clauses, body items, expressions, statements, patterns,
types, scopes, locals, source components, diagnostics, and candidate rows.

Failure, cancellation, panic, stale/foreign input, and one-over limits must
publish no partial item, ID, scope, local, source row, diagnostic, candidate,
result, or invalidation fact.

### 6. Current consumer deletion inventory

Name every current producer and consumer from inspected `main`, with path,
owner, old authority, final typed authority, and migration order. The public
switch must migrate syntax, HIR, sema, compiler, verifier, runtime-plan,
formatter, LSP, CLI, Agent/debug, project publication, cache, and tests while
deleting old clone-HIR, value-tail Flow bodies, detached readers, and
source-string reconstruction in the same compiling authority cut.

Do not repair an old Flow reader. Do not preserve it behind an adapter,
wrapper, alias, deprecated export, dual reader, optional fallback, or V2 API.

## Required test contract

Provide exact positive, malformed, negative, recovery, compile-fail,
stale/foreign, cancellation, panic, rollback, exact-limit, first-one-over,
multi-module, incremental-reorder, source-query, and consumer-migration tests.
The matrix must cover every identity state, signature row, contract row,
body-state row, and `HirThreadFlowItem` row in both Flow and Thread contexts,
including explicit non-applicable combinations.

Tests must use typed APIs, observable behavior, codecs, or structured
dependency evidence. Do not use source scans, symbol/file spelling gates, or
permanent diagnostics for removed unreleased syntax.

## Constraints

- Preserve accepted Proof arena IDs, transactions, source-query semantics,
  module-preserving project identity, Call, Select, and Dialogue application.
- Preserve accepted Lang-01.1.1 callable/effect identities without designing
  the later DirectFrame/StreamFactory runtime switch here.
- No compatibility aliases, wrappers, shims, deprecated APIs, dual readers,
  migration maps, copied projections, source-string reparsing, source gates,
  removed-syntax-specific permanent diagnostics, CSS, or Takumi.
- Do not retain raw clause text, syntax clones, copied source ranges, sentinel
  IDs/types/names, or consumer-local side tables.
- Do not redesign TTS, CharacterDialogue runtime/View/Agent/save, Stream,
  resource, manifest, runtime assertion, AWBC, or save/replay mechanics.

## Required returned archive

Return exactly one ZIP named:

```text
arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip
```

Put every sidecar inside the ZIP. Include at least:

- `README.md`;
- `FINAL_STATUS.md`;
- `OPEN_QUESTIONS.md`;
- verbatim copies of the parent, primary, and this correction request;
- `PRECEDENCE_AND_NON_GOALS.md`;
- repository/predecessor ledger with exact revision and hashes;
- exact attached-syntax and final-HIR Rust schemas;
- complete identity/signature and contract-clause tables;
- complete Flow/Thread body, scope, local, source, recovery, diagnostic,
  accounting, and limit tables;
- exhaustive `HirThreadFlowItem` matrix;
- current producer/consumer inventory and deletion-driven implementation order;
- complete test matrix and requirements traceability; and
- a sorted manifest containing every non-self member's exact byte length and
  SHA-256.

Do not require adjacent summary, status, hash, or manifest files. Do not return
a production patch or implementation overlay.

Use `READY_FOR_IMPLEMENTATION` only when `OPEN_QUESTIONS.md` is exactly `none`
and every result-changing row above is closed against current GitHub `main`.
If an intermediate pass finds omissions that current repository evidence can
resolve, continue the same assignment and resolve them before returning. A
generic contract that adopts this request without supplying its schemas and
matrices is not a valid READY return.
