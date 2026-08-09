# Proof final-HIR pattern-owner reconciliation

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores the pattern schema decisions and their acceptance mapping.
The protected checkout's WIP/PASS labels, test counts, structural measurements,
and revision identifiers are intentionally not retained.

## Closed pattern inventory

Attached syntax and final HIR use thirteen semantic families:

```text
Binding, MutableBinding, Literal, EntityReference, Variant, Discard, Tuple,
Record, BracketSequence, WholeBinding, Or, TypedBinding, Error
```

Known malformed syntax remains its family with typed poison. Generic Error is
reserved for syntax whose family cannot be classified or for transactional
child failure.

### WholeBinding and TypedBinding

Whole binding is `Ident NonBindingPattern`; it has Name and nested Pattern
components and no `@` component. `@` remains entity-reference syntax, so a
`WholeBindingAt` role is neither optional nor synthetic.

Typed binding is a Pattern family whose attached owner contains Name, Colon,
and one attached Type child. A Let statement passes its complete left side to
this pattern owner and has no second annotation reader. Callable, closure, and
declaration parameters remain different parent schemas: they own separate
pattern and type children before entering the Pattern transaction.

### Or binding identity

The first Or alternative establishes the canonical binding-position map and
local ordinals. Later alternatives reuse those ordinals and cannot add, remove,
or reorder positions. Binding-set disagreement is fatal
`OrAlternativeBindingsMismatch`, rolls back the transaction, and publishes no
Or or Local; it is not a recoverable HIR Pattern state. A valid Or has at least
two same-module, same-scope alternatives in source order.

A `(ScopeId, HirName)` generation may advance only when the new binding-name
source start is strictly later. Prior-or-equal source order is a fatal
transaction error. Duplicate bindings within one root and later Or alternatives
reuse the root allocation map and do not advance the generation.

The acceptance generator uses a nested destructuring fixture to prove preorder
ordinary-binding ordinals, separate record-rest ownership, exact Local reuse
across paired Or alternatives, and identity stability when typed child-map
insertion order is perturbed in the same database. This is semantic identity
evidence, not a source scan.

## Attached syntax and semantic projection

One parser transaction constructs the typed Pattern projection and exact
component/type-child map. An attached Pattern handle keeps the same semantic
owner across snapshot or fragment rebasing; its node path selects children
without detached parsing.

- Binding owns one significant name token; missing, invalid, or trailing input
  is typed recovery and cannot fabricate a Local.
- Paths retain implicit/crate/self/super/absolute root semantics and each
  segment's identifier versus project-symbol token family.
- Qualified and expected-type-relative Variant heads are distinct typed forms;
  no nominal spelling such as `Some` or `Ok` is hard-coded.
- A missing Variant name owns a typed zero-width insertion. A started payload
  with a missing close retains any successfully parsed child.
- Bracket-sequence rest has Absent, Unbound, Binding, and Recovered states. Only
  the first rest owns the slot; later occurrences are recovery evidence.
- Duplicate record fields and multiple record rests are cross-field HIR
  validation, because they require sibling-order comparison.

Path preflight applies to both resolved and recovered shapes: at most 256
segments, 1,024 name bytes per attempted segment, and 65,536 semantic path
bytes. Recovery cannot bypass a hard limit.

## Literal authority and limits

The lexer owner decodes values and their exact components once. Pattern and HIR
projection consume that typed result and never decode source text again.
Integer and decimal payloads retain arbitrary-width structures; Character uses
the canonical character spelling; malformed values retain a family-specific
issue.

The literal issue variant itself determines String, Character, Integer,
Decimal, UnitNumber, or Duration intent. Decimal punctuation/exponent markers
retain Decimal intent; radix prefixes retain Integer intent. Unknown suffixes
do not infer UnitNumber or Duration by prefix or edit distance. The unpublished
`Milli` UnitNumber suffix remains deleted with no alias or dedicated removed
syntax diagnostic.

The lexer-owned numeric digit count excludes prefixes, separators,
punctuation, and suffix/unit components. Final HIR consumes that count before
allocating arbitrary-precision values. Exact coefficient, scale, exponent,
digit, and Duration normalization bounds remain fallible typed constructors or
transaction limits; limit failure is rollback, never literal recovery.

Schema variants with no current authored producer remain explicit
not-applicable rows with typed mapping evidence. They are not exercised through
invented spellings.

## Required-field recovery

Name-owning families use a recovery-bearing binding payload:

```text
HirPatternBinding::Bound { name, local }
HirPatternBinding::Recovered { issue }
```

Missing or invalid names allocate no placeholder Local. Variant names,
entity-reference IDs, paths, and malformed literals similarly retain
resolved/recovered typed values. Required missing children remain typed
recovery IDs or state-consistent absent fields; foreign or wrong-kind IDs are
transaction failures, not source recovery.

Every recovered payload must agree with the pattern-specific poison issue.
Source freeze reconstructs the expected family, children, source roles,
bindings, and recovery from the same attached owner; family equality alone is
not enough to accept payload substitution.

## Acceptance traceability and deletion

The current Proof matrix must cover all thirteen families with exact payload,
source query, owner status, not-applicable roles, optional absence where
defined, ordinal bounds, known-family poison, and diagnostic ownership. It must
also cover source-reachable malformed literal rows, explicit not-applicable
schema rows, trivia relowering, stale/foreign attachment, exact/one-over limits,
Or mismatch rollback, and generator determinism. This document carries no
current PASS credit for those rows.

The public switch deletes detached Pattern models, `parse_pattern_at`, source
reparsing, provisional wrappers, and duplicate ID-reference owners, then fixes
all consumers against attached Pattern IDs. No forwarder, sentinel, alias,
dual reader, source gate, or removed-syntax diagnostic is admitted.
