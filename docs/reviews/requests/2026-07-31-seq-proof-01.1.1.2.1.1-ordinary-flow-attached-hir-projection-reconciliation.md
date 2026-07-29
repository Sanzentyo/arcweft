# Design request: proof-concurrency 01.1.1.2.1.1 ordinary Flow attached syntax and final-HIR projection reconciliation

- Date: 2026-07-31
- Sequence: proof-concurrency v6.1.1.2.1.1, split from the repository-resolved
  v6.1.1.2.1 final-HIR item/member inventory and required before the Flow
  item can join the Proof public HIR/project authority switch
- Kind: independently throwable, design-only correction contract
- Production changes: prohibited
- Output language: English
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation-final-contract.zip`

## Goal and authority

Return one decision-complete standalone contract for the ordinary source-level
`Flow` item from attached syntax through final HIR and the revision-bound source
index. Close only the remaining result-changing ownership and projection
decisions. Do not redesign semantics already accepted by the current language,
Proof-concurrency, AW-AH-009.4.2/.3, or Lang-01.1.1 contracts.

Use the latest GitHub `main`. Read, in full:

1. repository `AGENTS.md` and the applicable Rust skill;
2. this request and its parent gap-audit request
   `docs/reviews/requests/2026-07-25-seq-proof-01.1.1.2.1-final-hir-item-member-inventory-reconciliation.md`;
3. `docs/implementation/2026-07-26-proof-hir-local-schema-decisions.md`;
4. `docs/implementation/2026-07-17-proof-concurrency-v6-1-1-stage-1-flow-header.md`;
5. the latest Proof final-HIR scope, source-owner, tail-owner, Call, Select,
   Predicate, Proof, Function, and public-switch implementation/intake notes;
6. every accepted Proof v6.1.1 package retained under `docs/reviews/packages/`
   and `docs/reviews/designs/` that those notes identify as authoritative;
7. the accepted Lang-01.1.1 contract packages and their intake notes, including
   the effect/callable contract chain;
8. `docs/01-language/grammar.md`, `docs/01-language/contracts.md`,
   `docs/01-language/block-scopes.md`, `docs/01-language/syntax.md`, and the
   maintained Flow/dialogue/control-transfer chapters; and
9. the current attached-syntax, final-HIR, source-index, semantic, compiler,
   runtime-plan, and LSP readers for Flow and thread/Flow-item bodies.

Record the exact Git commit inspected and the relevant repository evidence in
the returned package. A package prepared from an older revision must reconcile
all result-changing differences visible on current `main`.

This is a design-only assignment. Do not edit production code, tests,
manifests, stable design chapters, or fixtures. Do not create a branch, patch,
PR, implementation overlay, or migration crate.

## Accepted boundary that must not be redesigned

The following decisions are already fixed:

- `Flow` is one source item in the accepted `TypedItemNode` and final
  `HirItemKind` inventories.
- A Flow retains an optional typed public/entity ID, an optional ordinary name,
  typed generics and one optional fixed-parameter group, typed `where` and
  contract data, a callable scope, and an ordered `HirThreadBody`.
- A Flow body is statement/Flow-item-only and has no value tail. An empty
  authored body is valid and evaluates to semantic `Unit`; only an absent
  required body is `MissingBody`.
- A Flow is not a curried callable. A second parameter group is recovery.
- The scope chain begins at the module root and contains one item-owned callable
  scope, distinct contract scopes, and one item-owned Flow body scope. Exact
  child order and source ownership must be frozen from the attached tree.
- `HirThreadBody` is the shared final ordered body record used by ordinary
  Flow and Thread expressions. It contains typed `StmtId` values or the accepted
  typed dialogue-application `ExprId`; it contains no syntax clone, block
  `ExprId`, copied range, source text, or tail.
- Dialogue lines/content calls do not survive as legacy Flow carriers. They
  become the AW-AH-009.4.2 typed dialogue-content application owner, with
  AW-AH-009.4.3 line identity, during the same public authority switch.
- Qualified HIR IDs, arenas, source queries, accepted project generation,
  rollback, inclusive limits, and stale/foreign rejection remain exactly those
  of the accepted Proof-concurrency chain.
- The migration is deletion-driven. The public switch deletes old clone-HIR,
  detached readers, source-string reparsing, and obsolete Flow consumers in the
  same compiling authority cut.

Do not reopen these decisions merely because the current private WIP is
incomplete.

## Why this correction is required

The locally resolved v6.1.1.2.1 note fixes the outer Flow payload but does not
define the exact attached owner and source-frozen projection needed to construct
it. Current repository evidence exposes incompatible provisional choices:

1. The private Flow parser retains ordinary names and entity-reference tokens,
   but there is no decision-complete attached carrier for the four recovery
   states: ordinary name, typed ID, typed ID plus ordinary name, and missing
   identity. Re-reading the source spelling later is forbidden.
2. The private parser byte-preserves `invariant`, `assume`, `reads`, `effects`,
   `modifies`, and `decreases` without typed clause nodes. Maintained language
   design also names `no_effect` under contract semantics. The accepted final
   HIR currently models only `requires` and `ensures`, so scope, result
   visibility, typed payload, source role, clause ordering, and recovery are not
   defined for the full family.
3. The accepted thread contract enumerates `HirThreadFlowItem`, but the exact
   attached syntax owner and child payload are not closed for every variant.
   In particular `Choice`, `Select`, `SourceLocale`, `Scope`, `Include`, and
   `AwaitWith` must not be guessed from legacy AST structs or text.
4. The current private Flow body calls the ordinary value-block parser. That can
   classify a terminal expression as a tail, contradicting the accepted
   statement-only/no-tail Flow body.
5. The provisional final `HirFlowItem` requires an authored `TypeId` and rejects
   the state where both ID and name are absent. That conflicts with the accepted
   omitted-return semantic `Unit` boundary and with a recognized poisoned Flow
   retaining `MissingName` recovery rather than becoming a generic error item.
6. No complete variant-by-variant matrix fixes allocation order, primary issue
   precedence, exact/one-over accounting, source roles, scope parents, and
   rollback for ordinary Flow.

These are result-changing schema questions. Implementers must not resolve them
with a new temporary carrier, placeholder ID/type, compatibility reader, or
source-text interpretation.

## Required decisions

### 1. Exact attached Flow declaration owner

- Define the exact Rust-level attached records/enums, fields, visibility, and
  constructors for `FlowItem`, its header, identity, one optional parameter
  group, return annotation, clauses, body, and trailing recovery.
- Represent each identity form exactly: ordinary name, typed entity/public ID,
  typed ID plus ordinary name, and missing identity. State which form supplies
  semantic callable identity and presentation name without fabricating either.
- Define every source component and role, including keyword, ID, name,
  parameters, return, clauses, body, missing insertions, and recovery.
- Preserve parser-selected typed identity. Prohibit token-label matching,
  substring reparsing, split-on-dot helpers, diagnostic-code inference, and
  detached AST fallback.

### 2. Signature, omitted return, and result-local authority

- Define the final Flow signature record and whether an authored return is
  stored as `Option<TypeId>` or by another already accepted representation.
- Preserve an omitted annotation as semantic `Unit` without inventing a
  forbidden synthetic role, fake source node, sentinel `TypeId`, or copied
  `Unit` syntax.
- Define when an `ensures` result local exists, its exact `ScopeId`, annotation
  when the return was omitted, visibility to every clause family, allocation
  order, source origin, and poison behavior.
- Define zero-parameter fallback anchors and the exact recovery for a second
  parameter group, missing parameter type, missing return type after `->`, and
  malformed `where` rows.

### 3. Complete Flow contract-clause family

For `requires`, `ensures`, `invariant`, `assume`, `reads`, `effects`,
`no_effect`, `modifies`, and `decreases`:

- determine whether each is an admitted Flow header clause under the accepted
  language contracts, and state non-applicability explicitly where it is not;
- define one exhaustive attached enum and one exhaustive final-HIR typed payload
  mapping, including mode, proof/effect/path/value references, optionality,
  ordering, duplicates, and recovery;
- define the expression/type/path owner for every payload without retaining raw
  clause text;
- define the lexical scope of every payload, parameter/result visibility, and
  deterministic source order when clause families interleave;
- define source roles, primary and related source sites, diagnostics, poison
  precedence, and exact `ContractClauses` accounting; and
- reuse the accepted callable/effect/proof identities and registries. Do not
  create a Flow-only effect resolver, proof catalog, path parser, or copied
  semantic row.

Do not redesign the accepted semantics of these clauses. This request only
closes their attached-syntax/final-HIR/source projection.

### 4. Flow scope graph and body ownership

- Publish the exact scope kinds, owners, parents, source-ordered child slices,
  local visibility, and source origins for callable, every contract scope, and
  Flow body.
- Decide whether auxiliary clause families use existing contract scopes or
  require a closed additional scope vocabulary; justify the choice against the
  accepted scope graph rather than adding provisional scopes.
- Specify a Flow-specific statement-only body parser/attachment boundary that
  can share statement grammar without exposing a value tail.
- Define empty, missing, unclosed, and poisoned body records and the exact
  synthetic insertion role when one is already admitted. Do not add an
  optional tail merely to represent absence.

### 5. Exhaustive `HirThreadFlowItem` projection

Give a normative row for every final variant:

```text
Statement, DialogueApplication, Choice, If, IfLet, Match, Loop, While,
WhileLet, For, Select, SourceLocale, Scope, Include, AwaitWith, Error
```

For each row define:

- exact admitted attached `SyntaxKind`/typed node and contextual grammar;
- whether the child is a `StmtId` or `ExprId`, and the exact child payload;
- scope owner/parent, local visibility, source role and ordinal;
- lowering of nested statements/expressions and recovery children;
- empty/missing/malformed behavior and primary issue precedence;
- whether the same row is admitted in both ordinary Flow and Thread bodies;
  where contexts differ, define typed admission rather than a string flag; and
- exact HIR/source-index freeze validation that detects a substituted,
  duplicated, reordered, stale, foreign, or rolled-back child.

Reconcile the list with current maintained grammar. If a named variant has no
accepted authored surface, delete or classify it explicitly rather than
inventing syntax. If maintained grammar has a typed Flow item missing from the
list, define the direct final replacement rather than routing it through
`Statement` by convenience.

### 6. Poison, limits, and transaction freeze

- Define one deterministic Flow item primary-issue precedence across common
  prefix, identity, signature, clauses, missing body, body items, unclosed body,
  and trailing recovery.
- Define parser and HIR preflight accounting for identities, generics,
  parameters, clauses, body items, nested children, scopes, locals, source
  components, and diagnostics. Existing inclusive global limits remain the
  sole limits.
- Give exact-limit and first-one-over outcomes. Failure must publish no partial
  item, child ID, scope, local, source row, diagnostic, candidate, result, or
  invalidation fact.
- Define source-freeze re-derivation from attached syntax for every field and
  ordered child. Raw arena allocation order must not replace source order.

### 7. Public authority and deletion order

Provide one compile-clean deletion-driven sequence that:

1. privately installs the exact attached Flow owner and final-HIR/source freeze;
2. makes obsolete Flow identity/header/body readers unavailable first;
3. fixes every compile failure toward the final typed owner;
4. connects the module-preserving accepted project and sole callable/effect
   registries;
5. migrates sema, compiler, runtime-plan, verifier, formatter, LSP, CLI, Agent,
   and test consumers; and
6. switches publication while deleting the old parser/HIR/project readers in
   the same public authority cut.

Name the current consumers to migrate from current `main`. Do not preserve an
obsolete consumer by adding an adapter, alias, wrapper, copied projection,
string key, legacy reader, or temporary V2 API.

## Required test matrix

The returned package must specify direct behavior/API evidence for at least:

- all four identity states and exact source components;
- zero and one parameter group, rejected second group, generic/lifetime
  parameters, destructuring locals, missing/recovered types, defaults if
  admitted, optional return, malformed return, and `where` recovery;
- every contract family alone and interleaved, duplicates, missing payloads,
  result visibility, proof/effect/path identity, exact/one-over clause limits,
  and rollback;
- empty, nonempty, missing, unclosed, and recovered Flow bodies with proof that
  no body tail exists;
- every `HirThreadFlowItem` variant in ordinary Flow and Thread context,
  including all explicitly non-applicable cross-context rows;
- Dialogue application through the accepted typed owner, without legacy
  speaker/content-call carriers;
- nested scope/local ordering, source-ordered children versus differing raw
  allocation order, stale/foreign IDs, and transactional rollback;
- omitted semantic Unit and exact `ensures` result-local behavior;
- typed diagnostics and one source query path through compiler/LSP/tooling;
- project-level same-name, ID/name collision, multi-module, incremental
  reorder, rejection, and accepted-generation identity cases;
- compile-fail/visibility evidence for removed constructors/readers and
  behavior evidence that no compatibility path executes; and
- focused syntax/HIR/sema/compiler/LSP tests, workspace check/Clippy/tests,
  applicable Tier 2, and structural audit.

Removal evidence must use typed APIs, compile-fail tests, behavior, codecs, or
structured dependency data. It must not scan checked-in source text for names,
paths, snippets, or forbidden spellings.

## Explicit non-goals

Do not include or block this package on:

- Lang-01.1.1 `DirectFrame`/`StreamFactory` runtime classification, direct
  suspension execution, generator runtime, scheduler, cancellation, AWBC, or
  save/replay mechanics;
- Trait/effect semantic public-switch implementation beyond reusing its
  accepted identities and defining Flow clause projection;
- CharacterDialogue runtime/View/Agent/save implementation;
- obsolete Dialogue runtime-carrier deletion beyond the already accepted
  typed dialogue-application handoff;
- TTS production work;
- Lang-01.3 Source-to-Stream, Lang-01.4 resource, or Lang-01.5.1 manifest
  implementation; or
- a redesign of Proof arena IDs, project generation, source query, or accepted
  Call/Select/Dialogue schemas.

## Constraints

- No compatibility aliases, wrappers, shims, deprecated APIs, dual readers,
  migration maps, or copied HIR projections.
- No source-string reparsing, string identity fallback, source gates, removed-
  syntax-specific permanent diagnostics, CSS, or Takumi.
- Do not repair or deepen the legacy Flow parser/HIR/runtime path. Specify its
  direct deletion and migrate consumers to final owners.
- Do not add a second expression, statement, path, proof, effect, callable,
  scope, local, project, diagnostic, or source-range authority.
- Do not use the current private implementation shape as authority when it
  conflicts with accepted contracts; identify it as migration evidence.

## Required returned archive

Return exactly one ZIP named:

`arcweft-proof-concurrency-v6.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation-final-contract.zip`

Put every sidecar inside the ZIP. Include at least:

- `README.md`;
- `FINAL_STATUS.md`;
- `OPEN_QUESTIONS.md`;
- copies of this request and the parent request;
- `PRECEDENCE_AND_NON_GOALS.md`;
- exact attached-syntax and final-HIR Rust-level schemas;
- a complete contract-clause table;
- a complete Flow-item lowering/source/scope/recovery/limit matrix;
- poison and diagnostic precedence;
- deletion-driven implementation order and consumer inventory;
- full test matrix and requirements traceability;
- repository evidence with inspected revision; and
- a sorted manifest containing every non-self member's byte length and
  SHA-256.

Do not require adjacent summary, status, hash, or manifest sidecars. Return no
production patch or implementation overlay.

Use `READY_FOR_IMPLEMENTATION` only if `OPEN_QUESTIONS.md` is exactly `none`
and every result-changing identity, clause, scope, Flow-item, recovery, source,
limit, transaction, and consumer-deletion row is closed. Otherwise return the
same correctly named ZIP with `NOT_READY` and explicit unresolved decisions.
