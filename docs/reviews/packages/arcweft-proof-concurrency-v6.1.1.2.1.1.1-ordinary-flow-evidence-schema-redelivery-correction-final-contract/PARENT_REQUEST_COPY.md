# Design request: proof-concurrency 01.1.1.2.1 final HIR item/member inventory reconciliation

> **RESOLVED LOCALLY — DO NOT DISPATCH.** The repository-owned decision is
> recorded in
> `docs/implementation/2026-07-26-proof-hir-local-schema-decisions.md`.
> This file remains only as gap-audit history.

- Date: 2026-07-25
- Sequence: proof-concurrency v6.1.1.2.1, after the accepted v6.1.1 base and
  v6.1.1.2 retained-global declaration contracts and before the public HIR item
  authority switch
- Kind: independently throwable, design-only correction contract
- Production changes: prohibited
- Output language: English
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.2.1-final-hir-item-member-inventory-reconciliation-final-contract.zip`

## Goal and authority

Return one decision-complete correction that fixes the exact final
`HirItemKind` inventory and defines the exact closed retained-declaration
member payload. Use:

1. latest Arcweft `main`, current `AGENTS.md`, and the applicable Rust skill;
2. proof-concurrency v6.1.1, SHA-256
   `1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`;
3. retained-global v6.1.1.2, SHA-256
   `0E30A91FA2F7A288E9A12D8AFC7356525604CBDC907D659CD97311207D26A68E`;
4. current accepted downstream package decisions visible in the repository; and
5. this request, copied into the returned archive.

Do not redesign the accepted syntax database, typed IDs, arenas, transactions,
retained declaration grammars, or project identity unless current evidence
proves a concrete contradiction. This request closes an item/member schema
conflict; it does not authorize implementation.

Dispatch this Markdown by itself to exactly one design assignee with access to
latest `main`, `AGENTS.md`, the Rust skill, and the two repository-retained
archives named above. Do not combine it with the statement request below; the
two requests may be designed concurrently by different assignees.

## Why a correction is required

The base archive calls its `HirItemKind` inventory exact, but includes
source-level variants such as `Agent`, `Callable`, `State`, `ExternModule`,
`Hook`, `DialogueDefaults`, `MemoFunction`, `Parser`, and `TopLevelFlow` for
which the accepted attached `TypedItemNode` inventory has no corresponding
source node. Several are already removed by current grammar, compile-fail
evidence, or later contracts. The retained-global archive also replaces the
base generic `EntityDeclaration` with concrete Character, View, Action,
Activity, Signal, Metric, and Layer items, and adds the independent typed
`Resource` item.

At current `main`, the attached item inventory is:

```text
Module, Use, Flow, Function, Predicate, Proof, Trait, Impl, Enum, Struct,
TypeAlias, Resource, Character, View, Action, Activity, Signal, Metric, Layer,
Entry, ExternCapability, Test, Bench, Source, Style, Error
```

The correction must adjudicate contract precedence and final ownership rather
than keeping source-less variants or inventing empty payloads.

Separately, `PUBLIC_AST_HIR_MIGRATION.md` names `DeclarationMemberId` fields
and says only that `HirDeclarationMember` is a closed enum for View exports,
Activity ports, Metric unit/labels/buckets, optional Character display-name
member ownership, and Layer members. It does not enumerate variants/fields and
leaves Character display-name representation as an implementation choice. No
other accepted ZIP supplies that schema.

## Required decisions: final item inventory

1. Publish one exhaustive final `HirItemKind` enum matching the final attached
   item authority. For every base/later variant, state `retain`, `replace`, or
   `delete`, with the authoritative syntax owner and exact reason.
2. Reconcile generic `EntityDeclaration` with the seven concrete retained
   declarations, and reconcile `Resource` with the separate Lang-01.4 owner.
3. Reconcile `Source` with the accepted Proof source node and the later
   Lang-01.3 atomic removal. Do not delete its only production path early and
   do not preserve it after the Lang-01.3 switch.
4. Reconcile every source-less base variant, including Hook/Memo/Parser and
   top-level-flow concepts, against current grammar and downstream owners. A
   synthetic/compiler product must not masquerade as a source `ItemId` unless
   the contract defines its exact non-source origin and liveness.
5. For every source-less, removed, replaced, or synthetic base variant, define
   its exact deletion/replacement outcome and any surviving non-source owner.
   Do not redesign ordinary attached Module/Use/Flow/Function/Predicate/Proof/
   Trait/Impl/Enum/Struct/TypeAlias/Entry/ExternCapability/Test/Bench/Source/
   Style/Error payloads merely because their current syntax-clone fields must
   be replaced mechanically by typed IDs; that migration is locally
   determined by existing semantics and the accepted arena rules.
6. Define poison/error item ownership and prohibit fabricated valid items for
   attached `ErrorItem` or missing required children.

## Required decisions: declaration members

1. Define the exact Rust-level record/enum behind `DeclarationMemberId`, every
   variant, exact fields, optionality, ordering, and public resolver return.
2. Define View export payloads without dotted-string splitting.
3. Define Activity input/output port direction, name, type, and local ownership.
4. Define Metric unit, label, and ordered bucket payloads.
5. Select one Character display-name representation and reconcile the accepted
   direct `Option<ExprId>` with the conditional member-source-slot sentence.
6. Define every Layer reference/policy/expression member using accepted owned
   enums and typed references, never keyword strings or a generic map.
7. Specify member source slots, `SyntaxNodeId` origin, liveness, limits,
   recovery/poison, and transactional rollback.

## Migration, consumers, and constraints

Specify a deletion-driven order that privately installs exact item/member
records, lowers attached nodes by `SyntaxNodeId`, migrates project/sema/
runtime-plan/verifier/formatter/LSP/CLI/Agent consumers, and deletes every
generic/string/cloned/source-less obsolete path in the same public authority
switch. Do not add a compatibility wrapper, alias, dual reader, syntax-AST HIR
payload, removed-syntax diagnostic, or source gate.

Independent expression/type/pattern/statement HIR work may continue while this
contract is returned. Implementers must not guess the blocked item/member rows.

## Tests to specify

Define direct tests for exhaustive item projection, every retained member
variant, same-kind sibling identity/order, poisoned/missing children, stale or
foreign IDs, rollback and exact/one-over limits, project/sema consumer parity,
and compile-fail absence of raw constructors/generic members/source-less old
variants/compatibility APIs. Language removal must use ordinary parser/compiler
rejection and absence of an executable typed node, never source scanning.

## Required returned archive

Return one ZIP with all sidecars inside it. Include at least `README.md`,
`FINAL_STATUS.md` (`READY_FOR_IMPLEMENTATION` or `NOT_READY`),
`OPEN_QUESTIONS.md`, a copy of this request, exact item/member schemas,
precedence and deletion tables, implementation order, full test matrix,
requirements traceability, and a manifest with every non-self member's byte
length and SHA-256. Return no production patch, overlay, compatibility code, or
required external sidecar. If one exact inventory cannot be selected, return
`NOT_READY` and name the blocking decision.
