# Design request: Proof-concurrency 01.1.1.4.1.1.1.1.1 Select recovered-member schema correction

- Date: 2026-07-28
- Sequence: Proof-concurrency v6.1.1.4.1.1.1.1.1, narrowly correcting
  the accepted E13 `Select` payload/matrix contradiction
- Kind: independently throwable, GitHub-only, design-only correction
- Production changes: prohibited
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1-select-recovered-member-schema-correction-final-contract.zip`

## Assignment

Prepare one decision-complete correction for the E13 `Select` expression row
in the latest GitHub `main` of:

`https://github.com/Sanzentyo/arcweft`

Read repository `AGENTS.md`, this complete request, the accepted tail package,
its intake, the accepted source-owner and leaf-expression packages, and the
current final-HIR expression schemas and implementation decision:

- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`
- `docs/implementation/2026-07-28-proof-01-1-1-4-1-1-1-1-tail-owner-generator-intake.md`
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`

The exact WIP payload conflict is reproduced in this request. Do not require
access to an unpublished working-copy implementation note or Rust overlay.

This request does not reopen E01-E12, E14-E35, qualified typed HIR IDs,
attached syntax ownership, source-query semantics, transaction atomicity, or
deletion-driven migration. Correct only the result-changing E13 contradiction
below and restate every affected E13 schema and matrix row completely.

## Why a correction is required

The accepted final payload is exactly:

```rust
pub struct HirSelectExpr {
    target: ExprId,
    member: HirName,
}
```

`HirName::try_new` accepts only a valid authored identifier. There is no
recovered-name carrier in `HirSelectExpr`.

The accepted E13 lowering and test rows simultaneously require all of the
following:

- a missing target or member remains a known `Select` family;
- the missing target/member produces `RecoveryOperand/name poison`;
- known-family poison must not collapse to generic `HirExprKind::Error`; and
- no valid name, range, identifier, or sentinel may be fabricated.

The missing-member case cannot satisfy those requirements with the accepted
payload. Constructing an empty or invented `HirName` is prohibited; changing
the missing-member result to generic Error would contradict the matrix. The
implementer therefore cannot choose a result without changing normative
behavior.

## Required decisions

### 1. Exact recovered-member representation

Choose and specify exactly one authoritative outcome:

1. revise the `Select` payload so its member field has a typed
   resolved/recovered representation that can carry a missing or invalid
   member without fabricating `HirName`; or
2. revise the E13 recovery matrix so missing-member syntax does not retain a
   `Select` payload.

If option 1 is selected, provide the complete Rust schema, constructor
invariants, public/private visibility, equality/hash behavior, exact recovery
issues, and accessors. State whether invalid authored text and absent text are
distinct. Reuse an existing final-HIR name recovery type only if its semantics
and source ownership are exactly applicable; do not introduce an alias or
wrapper around an incompatible provisional type.

If option 2 is selected, give the exact final payload and diagnostic outcome
for missing and invalid members, and explain how it remains consistent with
the known-family/generic-error rule.

### 2. Exact source and synthetic ownership

Restate the complete E13 source-role table for:

- `Whole`;
- `Target`;
- `SelectedMember`; and
- every applicable insertion or recovery coordinate.

Specify the missing-target and missing-member `SyntheticKey` owner, role, and
ordinal, or explicitly state that no synthetic child exists for a recovered
member value. Preserve source revision identity and distinguish `Span`,
`Insertion`, optional absence, and role-not-applicable. Do not encode a source
range inside a fabricated name.

### 3. Lowering, limits, and rollback

Give the exact lowering order from attached syntax to the selected payload,
including allocation, recovery diagnostic, source-manifest staging, poison
binding, and transaction publication. State exact and one-over work charges
for clean, missing-target, missing-member, and invalid-member cases. A hard
failure must publish no partial ID, source row, diagnostic, candidate, result,
or work fact, and retry must reuse the same qualified identity without
duplicating diagnostics.

### 4. Complete E13 evidence matrix

Provide standalone `T-E13`, `T-Q-13`, and `T-RB-13` rows covering at least:

- clean `target.member`;
- missing target;
- missing member;
- invalid authored member;
- exact payload and poison issue;
- every applicable source role and one inapplicable role;
- stale revision, foreign document/module, retired owner, accepted synthetic
  insertion, rollback, and retry;
- exact and one-over allocation/work boundaries; and
- compile-fail evidence that raw construction, Serde, aliases, wrappers, and
  compatibility readers are absent.

Tests must exercise `ParsedSource -> attached semantic node -> staged final
HIR transaction -> source query -> commit/rollback`. Hand-constructed payload
tests alone are insufficient.

## Constraints

- Design only: do not edit production code, create a patch, branch, PR, or
  implementation overlay.
- Preserve all accepted non-E13 expression schemas and matrices.
- Do not fabricate a valid `HirName`, identifier, `ExprId`, range, or sentinel.
- Do not add aliases, wrappers, extension traits, compatibility shims, dual
  readers, source-string reparsing, source gates, CSS/Takumi paths, or
  permanent removed-syntax-specific diagnostics.
- Do not repair the detached `Expr`, old HIR model, `SpeakerLine`,
  `ContentCall`, or stringly `HirDialogue` paths.
- The eventual implementation remains deletion-driven: migrate consumers to
  the selected final owner and delete obsolete readers rather than preserving
  both paths.

## Required return

Return exactly one ZIP with every sidecar inside and no required adjacent
files:

```text
arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1-select-recovered-member-schema-correction-final-contract.zip
```

Include README, this complete request copy, `FINAL_STATUS.md`,
`OPEN_QUESTIONS.md`, predecessor precedence, complete affected Rust schemas,
the E13 lowering/source/synthetic-owner tables, implementation and deletion
order, full focused test matrix, traceability, repository evidence, validation
report, and manifest.

Use `READY_FOR_IMPLEMENTATION` only when `OPEN_QUESTIONS.md` is exactly
`none` and the clean, missing-target, missing-member, and invalid-member cases
all have one non-fabricated final payload, source identity, recovery issue,
limit/accounting rule, and atomic migration path. Otherwise return the same
correctly named ZIP with `NOT_READY` and explicit unresolved decisions.
