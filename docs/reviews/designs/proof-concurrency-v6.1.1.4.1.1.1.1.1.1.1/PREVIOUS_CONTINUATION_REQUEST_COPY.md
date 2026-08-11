# Design request: Proof-concurrency 01.1.1.4.1.1.1.1.1.1 Select source, diagnostic, and producer authority correction

- Date: 2026-07-29
- Sequence: Proof-concurrency v6.1.1.4.1.1.1.1.1.1, continuing the rejected
  E13 Select recovered-member return
- Kind: independently throwable, GitHub-only, design-only redelivery
- Production changes: prohibited
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction-final-contract.zip`

## Assignment

Continue the E13 Select design work and return one decision-complete standalone
replacement in the latest GitHub `main` of:

`https://github.com/Sanzentyo/arcweft`

Read repository `AGENTS.md`, this complete request, the primary E13 request,
the rejected-return intake, every referenced accepted predecessor archive and
intake, and the current public syntax/source/diagnostic/limit evidence.

Required repository documents include:

- `docs/reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1.1-select-recovered-member-schema-correction.md`;
- `docs/implementation/2026-07-29-proof-01-1-1-4-1-1-1-1-1-select-return-intake.md`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`;
- `docs/implementation/2026-07-28-proof-01-1-1-4-1-1-1-1-tail-owner-generator-intake.md`;
- the parser's current Select/ShortVariant/error recovery and budget owners;
- the attached expression projection and final HIR source-role applicability;
  and
- the sole recovery-diagnostic schema and module-freeze obligation logic
  reproduced by the rejected intake.

Open the predecessor ZIP members directly and record their actual hashes and
the schemas/matrix rows used. If those repository archives cannot be read,
return `NOT_READY`; do not infer them from filenames or from the rejected ZIP.

The rejected archive is identified by SHA-256
`BC190B94A0DD285A4AA786ED076568FD9633C063648B2A171C869888F1D7BE38`.
The intake reproduces every blocking result, so the archive is optional input.
Reuse its usable analysis if available, but do not return a delta that depends
on it and do not merely change its status or filename.

This is a design-only assignment. Do not edit production code, create a patch,
branch, PR, or implementation overlay.

## Preserved useful decision

Preserve the direct original-owner replacement unless direct evidence exposes
a new concrete flaw:

```rust
pub enum HirSelectedMember {
    Name(HirName),
    Missing,
    Invalid(HirNameInvariantError),
}

pub struct HirSelectExpr {
    target: ExprId,
    member: HirSelectedMember,
}
```

Keep the fields private, construction crate-owned, access read-only, and
comparison/hash/order structural. `Missing` and `Invalid` remain distinct.
Final HIR stores no source spelling, range, revision, sentinel, or synthetic
member ID. Change this schema only if the complete corrected producer/source/
diagnostic matrix proves it cannot satisfy an invariant.

## Why continuation is required

The rejected return was mechanically valid, but its implementation matrix
cannot pass the accepted authorities:

1. it added a `Recovery` component to known Select roots and synthetic
   children even though E13 owns `Target`/`SelectedMember` and synthetic
   insertion is slot `Whole`;
2. it required two root/role diagnostics on one Select although recovery
   diagnostics are unique by qualified owner and a `RecoveryOperand` child is
   the terminal missing-child diagnostic owner;
3. it named no real ParsedSource spelling or attached projection capable of
   producing missing-target or invalid-member Select;
4. its parser 256/257 evidence is shadowed by the 128/129 diagnostic limit;
5. it did not count retained syntax diagnostics in the final HIR diagnostic
   limit;
6. it did not isolate `Expressions` from `TotalSlotsPerModule` one-over;
7. it omitted source/query behavior for authored poisoned targets; and
8. it prescribed an error-A/error-B diagnostic mismatch even though the
   diagnostic schema has no error payload.

The replacement must repair these exact issues rather than restating the
previous package.

## Required decisions

### 1. Exact ParsedSource and attached Select producer

Define the complete typed parser-to-attachment owner for E13 before deleting
any detached reader. Give exact Rust schemas, derives, visibility, invariants,
and accessors for the attached Select target and selected-member states. The
projection must retain, without reopening source text:

- the authored Select delimiter and exact `Whole` span;
- the target's authored identity or one exact grammar recovery state;
- `Name`, `Missing`, and `Invalid` member states;
- exact `Target` and `SelectedMember` span/insertion components; and
- the source revision and attached `SyntaxNodeId` already owned by the common
  attached node.

For every clean and malformed state, provide literal current-Arcweft source
fixtures, token boundaries, CST kind/roles, parser diagnostics, attachment
projection, and final-HIR result. In particular:

- leading `.member` is currently ShortVariant and must not silently become a
  missing-target Select;
- postfix Select currently requires a parsed left expression;
- `target.` is the missing-member fixture only if its exact recovery shape is
  specified; and
- a non-name token after the delimiter is not an Invalid member unless the
  grammar consumes it into the Select at a specified boundary.

For missing target, choose exactly one evidence-backed outcome:

1. identify an existing, unambiguous current-language source spelling and
   define its complete attached Select recovery path; or
2. prove no current grammar producer exists and correct the E13 matrix so a
   missing target is not claimed, rather than adding a test-only producer or a
   new language spelling.

Make the same reachability decision for invalid authored member. Do not invent
syntax solely to preserve a predecessor recovery row. Hand-constructed
projection values may test constructor invariants but cannot satisfy the
required end-to-end evidence.

### 2. Preserve the sole source-query authority

The final public authority remains the accepted
`HirModule::source_site(expected_source, HirSourceQuery)` over one immutable
source index. `Whole` remains arena-slot metadata.

For a known Select root, use only the exact applicable component roles selected
by its final shape. The retained default is:

```text
Whole             slot-owned Select span
Target            required Span or reachable recovery Insertion
SelectedMember    required Span or missing-member Insertion
Recovery          role not applicable to Select
```

A synthetic missing-target child, if reachable, owns its insertion as slot
`Whole`; do not add a duplicate source-backed `Recovery` component. Generic
source-backed Error remains the owner of the `Recovery` role.

Provide the full E13 applicability/requiredness/query matrix for clean,
missing, invalid, authored-poisoned child, every reachable combined state,
stale revision, wrong document/length, foreign module, not-yet-live/retired
owner, rollback, and retry. State validation precedence. Do not add an E13 map,
stored role outcome, fallback role, raw range reader, or second query API.

### 3. Reconcile terminal diagnostic ownership and singular root poison

Preserve one qualified owner as the recovery-diagnostic identity unless a
complete predecessor-backed replacement is unavoidable. Do not use an
undeclared `(owner, role)` key.

At minimum, close these event rules:

- a synthetic `RecoveryOperand` child is the terminal owner of its missing
  child event, with the child's slot `Whole` as primary;
- propagation-only parent poison does not duplicate that event;
- a Select's own missing/invalid member event is root-owned with
  `SelectedMember` as primary;
- a poisoned authored target uses the retained roleful authored-child
  propagation issue rather than copying an arbitrary child issue into a
  different semantic family; and
- each reachable target/member combination has a deterministic set and order
  of diagnostics with no duplicate owner.

If target poison and member recovery coexist, define exactly:

- the singular `HirPoisonState` issue and precedence;
- which terminal owners require HIR recovery diagnostics;
- whether a root obligation remains for the independent member payload;
- how module freeze validates that obligation from typed Select payload;
- how syntax diagnostics and HIR recovery diagnostics contribute separately;
- how a diagnostic renderer derives the correct member versus target message;
  and
- retry/dedup identity.

A correction may use the target child plus Select root as two distinct owners.
It may not stage two `HirRecoveryDiagnostic` records for the same owner without
providing and justifying a complete global schema/migration, which is outside
the preferred narrow correction.

Replace the impossible “diagnostic error A versus error B” test with direct
payload/state/primary/source/obligation invariants that exist in the selected
schema.

### 4. Exact work accounting and reachable limits

Provide case-by-case checked deltas for:

- parser recovery records;
- parser syntax diagnostics;
- final HIR recovery diagnostics;
- final total diagnostics;
- expression slots;
- total module slots;
- synthetic descendants, if a missing-target producer remains;
- attempted member-name bytes; and
- committed source components.

The parser's E13 recovery/diagnostic producer increments the current counters
together. Do not claim a ParsedSource E13 fixture reaches recovery 256/257 when
diagnostics reject at 129. Test the reachable boundary and delegate any
shadowed general maximum to direct accepted production-generator evidence,
with `NOT_APPLICABLE_WITH_EVIDENCE` rather than a fabricated fixture.

The final `HirLimit::Diagnostics` fixture must include retained parser syntax
diagnostics plus HIR recovery diagnostics. Give exact and one-over totals for
every malformed case.

Test `HirLimit::Expressions` and `HirLimit::TotalSlotsPerModule` independently:
leave the non-target budget with sufficient room, state deterministic preflight
order, and assert each exact owning error and complete rollback separately.

Use one shared attempted-name byte rule for valid and invalid authored name
attempts. Missing text charges zero; do not create an E13-only exception to the
existing typed-name preflight. State exact 1,024/1,025 behavior for both valid
and invalid attempts, plus source-document bytes and arithmetic overflow.

### 5. Complete corrected matrices and deletion switch

Return standalone `T-E13`, `T-Q-13`, and `T-RB-13` matrices whose detailed rows
exercise:

```text
ParsedSource
-> attached typed Select projection
-> staged final HIR/source/diagnostic transaction
-> source query
-> commit or rollback
```

Cover every reachable clean, missing, invalid, authored-poisoned-target, and
combined case; exact payload/member/root issue; every applicable role and one
inapplicable role; source identity/liveness failures; independently reachable
limits; rollback; retry; diagnostic dedup; and perturbation of internal work
order.

If a predecessor row is unreachable in the current grammar, replace it with
the explicit grammar evidence and remove downstream synthetic/accounting
claims that depended on it. Do not keep an impossible row merely to report a
larger matrix.

Provide compile-fail or structured public-API evidence for private raw
construction, no public Serde, and removal of the old member field/accessor
shape. Use behavioral tests, typed queries, compile-fail cases, and one-off
review evidence. Do not create a checked-in source gate.

The public switch remains deletion-driven. In one compiling authority change:

- change the original `HirSelectExpr.member` field directly;
- migrate every final-HIR consumer to `HirSelectedMember`;
- publish the attached projection and sole source/diagnostic owner;
- delete HIR-facing detached Select readers, source fallbacks, old constructors
  and old match arms; and
- leave only the parser-internal clean `SelectExpr` if it remains a genuine
  syntax implementation detail rather than a compatibility reader.

## Constraints

- Preserve all accepted non-E13 schemas and matrices.
- Do not fabricate a valid `HirName`, `ExprId`, range, source spelling,
  diagnostic field, recovery fixture, or sentinel.
- Do not add aliases, wrappers, extension traits, compatibility shims, dual
  readers, source-string reparsing, source gates, CSS/Takumi paths, or
  permanent removed-syntax-specific diagnostics.
- Do not restore or repair detached HIR, static Capacity string dispatch,
  `SpeakerLine`, `ContentCall`, or stringly `HirDialogue` paths.
- Do not redesign the accepted source query or diagnostic owner globally only
  to make the rejected matrix compile.
- Continue deletion-driven migration; obsolete APIs are removed first inside
  the coherent local switch and compile errors are repaired toward the final
  typed owner.

## Required return

Return exactly one archive named:

```text
arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction-final-contract.zip
```

Put every sidecar inside the ZIP. Include README, this complete request copy,
the primary request copy, the rejected intake copy, `FINAL_STATUS.md`,
`OPEN_QUESTIONS.md`, predecessor precedence and direct member audit,
repository evidence, complete affected final and attached Rust schemas,
parser/CST/attachment fixtures, source-role/applicability table, diagnostic
owner/obligation/root-poison tables, work/limit/rollback tables,
implementation and deletion order, complete focused test matrices,
traceability, validation report, and manifest. Do not require adjacent summary,
status, hash, or manifest files.

Use `READY_FOR_IMPLEMENTATION` only when `OPEN_QUESTIONS.md` is exactly `none`,
every claimed state has a real ParsedSource producer or explicit unreachable
evidence, the sole source/diagnostic/limit authorities are preserved, all
reachable exact and one-over rows are independently testable, and the complete
deletion switch is implementable without a second reader. Otherwise return the
same correctly named ZIP with `NOT_READY` and explicit unresolved decisions.
