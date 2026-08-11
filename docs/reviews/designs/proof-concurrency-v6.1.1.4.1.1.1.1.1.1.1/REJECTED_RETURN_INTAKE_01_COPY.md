# Proof 01.1.1.4.1.1.1.1.1 Select return intake

Date: 2026-07-29

Status: `RETURNED_REJECTED_NOT_READY_FOR_IMPLEMENTATION`

## Archive identity and mechanical validation

The externally returned archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1-select-recovered-member-schema-correction-final-contract.zip
```

- byte length: `33,623`;
- SHA-256:
  `BC190B94A0DD285A4AA786ED076568FD9633C063648B2A171C869888F1D7BE38`;
- package baseline: `f5087621eb764d421f95c99ee05eaae3c5f2f4d2`;
- the newer repository `main` changes only Call correction documentation and
  does not change the production Select boundary;
- `21` unique flat members with no unsafe or duplicate path;
- `20` intentional non-manifest rows, all with exact byte length and SHA-256;
- `REQUEST_COPY.md`: byte-identical to the repository request;
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION` plus newline;
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`; and
- the three predecessor hashes match the repository-retained accepted
  archives.

The archive is mechanically valid. It is not copied into Git. This path and
digest are the retained identity of the rejected return.

## Accepted direction

The central payload choice is sound and should be preserved by a correction:

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

This directly replaces `member: HirName`, keeps missing and invalid authored
states distinct, stores no source range or spelling in final HIR, and creates
no compatibility record or synthetic member child. The proposed derives are
implementable because the retained `HirName` and `HirNameInvariantError`
already implement the required comparison and hashing traits.

The following direction also remains usable if a real ParsedSource producer
exists: a genuinely missing Select target uses the accepted
`SyntheticOwner::Expr(select_root) / RecoveryOperand / 0 / Expr` identity. A
correction must not invent a source spelling or test-only attached producer to
make that row reachable.

## Adjudication

The repository rejects the archive's self-status. Several normative rows
cannot be implemented through the accepted source, diagnostic, parser, and
limit authorities.

### `Recovery` is not a Select component role

The return makes `Recovery` a fourth required component of every poisoned
Select root and a second component of a missing-target synthetic child. This
conflicts with the accepted known-family source model:

- E13 owns `Whole` as slot metadata and the exact `Target` and
  `SelectedMember` components;
- `HirExprKind::Select` admits `Target | SelectedMember` only;
- the source-backed `Recovery` component belongs to generic
  `HirExprKind::Error`, not to a known family; and
- a synthetic recovery child owns its insertion as slot `Whole`. It does not
  acquire a duplicate source-backed `Recovery` component.

The correction must use `Target` or `SelectedMember` as the typed primary for
a source-backed Select diagnostic and the synthetic child's `Whole` for its
own terminal diagnostic. It must not add an E13 map, fallback role, or second
source reader.

### The diagnostic identity cannot represent the returned matrix

The return places the missing-target diagnostic on `(select, Target)` and, for
combined recovery, requires both `(select, Target)` and
`(select, SelectedMember)` on the same root. The retained diagnostic schema is
instead keyed by one qualified `SyntheticOwner`:

```text
HirRecoveryDiagnostic { owner, primary, primary_site }
```

Module freeze rejects a second recovery diagnostic for the same owner. A
poisoned `RecoveryOperand` child is the terminal diagnostic owner for that
missing-child event and removes the parent's propagation-only obligation. The
production missing-child lowerer already stages that child-owned diagnostic.

Consequently, the returned missing-target and combined rows necessarily fail
module freeze. The archive does not provide a replacement diagnostic schema,
obligation algorithm, or migration for a root/role key, and this narrow E13
correction is not permission to create a parallel diagnostic authority.

A corrected contract must distinguish syntax diagnostics from HIR recovery
diagnostics and close all combinations explicitly. The compatible direction
is:

- missing synthetic target event: child owner, child `Whole` primary;
- missing/invalid member event: Select root owner, `SelectedMember` primary;
- target plus independent member recovery: two different owners (target child
  and Select root), never two diagnostics with the same owner; and
- propagation-only parent poison: no duplicate missing-child diagnostic.

If the member remains independently recovered while a target child is also
poisoned, the freeze obligation rule must say exactly when the root remains an
obligation and how its member payload validates that diagnostic. A singular
root poison, secondary member state, diagnostic text, and deterministic
ordering must agree. The returned package does not close that rule.

The mismatch test in `T-E13-07` is also unconstructible: it asks for diagnostic
error A versus error B, but `HirRecoveryDiagnostic` has no error payload. A
correction must test a real payload/state/source-primary invariant instead of
an invented diagnostic field.

### The required ParsedSource producers are absent

The detailed tests refer to an unnamed “syntax recovery fixture” for a missing
target and an unnamed “non-name member token” for an invalid member. Current
grammar does not produce either claimed attached state:

- a leading `.member` is a `ShortVariant`, not a Select with a missing target;
- postfix Select starts only after an already parsed left expression;
- `target.` creates a missing-name node;
- a non-name token after `target.` is not consumed as an invalid member and is
  left for outer generic recovery; and
- the attached expression projection has no typed Select target/member
  carrier yet.

Therefore `T-E13-02` and `T-E13-04` cannot exercise the required
`ParsedSource -> attached Select -> final HIR` pipeline. The correction must
either define an exact current-language source spelling, token-consumption
boundary, CST roles, attached projection schema, and validation for each state,
or revise an unreachable recovery row with direct grammar evidence. It must not
add a new language spelling solely to satisfy a historical matrix and must not
use hand-constructed attached nodes as end-to-end evidence.

### Several work-boundary rows are unreachable or do not isolate an owner

The parser recovery/diagnostic boundary pair cannot run as written. The
production expression parser increments its recovery and diagnostic counters
together for these events. The diagnostic maximum of `128` therefore rejects
the 129th event before the recovery count can reach `256` or `257`. A test-only
counter injection is not ParsedSource pipeline evidence. The correction must
test only the reachable E13 delta and explicitly delegate a shadowed general
boundary to its accepted production-generator evidence.

The package also counts only E13 HIR recovery diagnostics, while the final
module diagnostic limit covers the complete retained syntax plus HIR recovery
diagnostic vector. Exact and one-over fixtures must include both contributions.

`HirLimit::Expressions` and `HirLimit::TotalSlotsPerModule` are combined into
one room-two/room-one test. The first deterministic failure masks the other
limit. Each owner needs a separate exact/one-over pair with the other budget
left available, plus an explicit failure order.

Finally, attempted invalid names must follow one shared typed-name accounting
rule. The correction must reconcile invalid Select member spelling with the
existing attempted-name byte preflight rather than silently making E13 the
only invalid-name family that charges zero.

### The authored-poisoned-target rows are incomplete

`SELECT_CASE_MATRIX.tsv` introduces poisoned authored target cases, but the
source matrix and detailed `T-Q-13` cases do not define them. The package also
copies the child's arbitrary issue into the Select parent instead of
reconciling the retained roleful authored-child propagation model. A
correction must give the exact parent issue, root diagnostic primary,
secondary member behavior, source queries, and ordering for authored-poisoned
target with clean, missing, and invalid members.

## Follow-up and implementation boundary

The standalone continuation request is
[Proof 01.1.1.4.1.1.1.1.1.1 Select source/diagnostic/producer authority correction](../reviews/requests/2026-07-29-seq-proof-01.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction.md).

The `HirSelectedMember` direction is retained, but E13's public switch remains
design-blocked until that correction is accepted. E14-E35 and other
decision-complete private final-HIR work remain independently implementable.
No old Select reader, source fallback, compatibility wrapper, or detached HIR
path may be restored while waiting.
