# Proof 01.1.1.4.1.1.1.1.2.1 Call authority return intake

Date: 2026-07-30

Status:
`RETURNED_PACKAGE_REJECTED_IMPLEMENTATION_UNBLOCKED_BY_REPOSITORY_DECISION`

Repository inputs:

- [01.1.1.4.1.1.1.1.2.1 correction request](../reviews/requests/2026-07-29-seq-proof-01.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction.md)
- [01.1.1.4.1.1.1.1.2 primary request](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1.2-call-recovered-argument-schema-correction.md)
- [rejected predecessor intake](2026-07-29-proof-01-1-1-4-1-1-1-1-2-call-recovery-return-intake.md)

## Archive identity and mechanical validation

The externally returned archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction-final-contract.zip
```

- byte length: `38,626`;
- SHA-256:
  `41BB47824B91072B17EF79B8C50249863977AF5D5854EBCF9E515849C9F24480`;
- audited baseline: `004ff3d69f241954eb808985878c348b165a815c`;
- `27` unique safe members;
- `26` intentional non-manifest rows, all independently verified by byte
  length and SHA-256;
- `FINAL_STATUS.md`: `READY_FOR_IMPLEMENTATION`;
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`.

All six repository-retained predecessor archives were independently hashed and
their internal manifests were recomputed. Their retained identities are:

| Authority | Bytes | SHA-256 | Manifest result |
|---|---:|---|---|
| Proof 01.1.1.4.1 leaf | 64,523 | `61E2EE166BFF158FE83DCF1484B7B9380A81F60D865377503400D27D238CC708` | 19/19 exact |
| Proof 01.1.1.4.1.1 source owner | 91,023 | `2BCD3F78EFB76442C2698A24251C4D874F7A941C5A8985649EA157100908A72E` | 23/23 exact |
| Proof 01.1.1.4.1.1.1 synthetic-role owner | 33,968 | `A9603B3CC758D95DADA69310F87A2DC26B7A2CE0EA8B6E0DE39DE4AA51E75024` | 17/17 exact |
| Proof 01.1.1.4.1.1.1.1 tail/generators | 50,036 | `69DC42FC7C985FED638D08D694ED301291A50AF3CEFA7117321D4219BE7E6471` | 22/22 exact |
| AW-AH-009.3.3.3.1 accounting | 30,748 | `060332BC62273C34F267F0F15767FE6BBD328BE177CB8035E83F210267AB0D41` | 9/9 exact |
| AW-AH-009.3.3.4 associated Capacity | 35,758 | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | 9/9 exact |

The archive is not copied into Git. This path and digest are its retained
identity.

## Packaging and evidence defects

The package is not the standalone replacement its own request requires.

| Member | Returned bytes | Repository bytes | Result |
|---|---:|---:|---|
| `CORRECTION_REQUEST_COPY.md` | 4,898 | 18,099 | summary, not a complete copy |
| `PRIMARY_REQUEST_COPY.md` | 2,163 | 15,719 | summary, not a complete copy |
| `REJECTED_RETURN_INTAKE_COPY.md` | 2,430 | 8,888 | summary, not a complete copy |

The repository originals are present and were read, so this packaging defect
does not require another return before repository adjudication. The predecessor
audit also records the source-owner archive as 63,092 bytes even though the
verified archive is 91,023 bytes. Its SHA-256 is correct.

Two claimed repository blob identities are wrong:

- `crates/arcweft-lang-hir/src/identity.rs` is claimed as
  `18cb62f57e1d70ec1c79a1b7587af4339d635fed`; at the audited baseline it is
  `2c5abea32ca7df642522b449af832064bd1dd1ce`;
- `crates/arcweft-lang-hir/src/dialogue_application.rs` is claimed as
  `b9c49c78220b934f2356a68132a32e49e987b384`; at the audited baseline it is
  `a7b061cd4cdf0732cfca53ed507184ba040446a7`.

The package's predecessor table contains representative findings rather than
the requested complete member inventory. The independent manifest audit above
supplies the missing mechanical evidence; it does not repair the result-changing
schema defects below.

## Accepted direction

The following decisions remain implementation input:

- final Call component source uses only `HirSourceIndex` and
  `HirModule::source_site`;
- ordinary arguments retain authored positional, named, or postfix-spread
  form and source order;
- explicit call type arguments are distinct from associated-receiver generic
  arguments;
- dot classification is value-first and nominal-second, while explicit `::`
  is nominal-only;
- bare generic arity failure uses project declaration metadata, checks retained
  arguments once, and invokes the shared resolver zero times;
- all other checked calls use the existing `resolve_call_target`, complete
  `CallTargetFacts`, `CallableLimits`, and signature projection;
- the semantic candidate ceiling remains 256 and the Proof limit of two is a
  projection over complete facts;
- cursor rules remain the existing R04/R05/R08/R09/R13/R14 behavior;
- migration is deletion-driven and must leave no detached final reader,
  compatibility layer, source reparser, or old Capacity dispatcher.

These directions are accepted only with the repository decisions below.

## Repository decisions that unblock implementation

No new redelivery request is created. Current grammar, dependency direction,
and the accepted attached/final owners determine the missing decisions without
guessing.

### 1. Only an actually recognized Call family lowers as Call

Known-family recovery begins only after the parser has an unambiguous Call
owner: an authored callee followed by a parenthesized argument list, an
authored associated receiver and separator followed by a terminal member/call
shape, or the current callback-block Call form. Recovery does not reinterpret
another valid grammar family merely to exercise a provisional enum branch.

The following package rows are `NOT_APPLICABLE_WITH_EVIDENCE` and are removed
from the implementation matrix:

| Package row | Fixture | Repository result |
|---|---|---|
| E12-002 / T-E12-002 | `(x)` | transparent grouped expression; never missing-callee Call |
| E12-008 / T-E12-008 | `::member(x)` | no authored left-hand receiver in the current expression grammar; ordinary parse failure, not associated Call |
| E12-011 / T-E12-011 | `Vec<I32> with_capacity(8)` | no associated separator; ordinary unexpected-token failure |
| E12-012 / T-E12-012 | `Vec<I32>..with_capacity(8)` | `..` is the current Range operator, not an invalid dot separator |

Consequently the final source-produced Call vocabulary does not retain
`MissingCallee`, `MissingAssociatedReceiver`, `MissingAssociatedSeparator`, or
`InvalidAssociatedSeparator` merely for these impossible rows. Their absence
is tested by parser behavior and by the lack of an executable typed Call node,
not by a source gate.

Once a receiver plus `.` or `::` has established the associated-member family,
missing or invalid member recovery remains reachable and typed. Malformed
receiver type syntax may retain an attached poisoned type child when the same
grammar transaction has an unambiguous terminal member/call boundary.
Likewise, a recognized argument or type-argument list may retain missing and
invalid slot components. These recoveries do not change another valid grammar
family.

`RecoveryOperand(0)` remains reserved by the closed semantic-role ordering for
the callee role, but current source does not generate it. Missing argument
values retain `RecoveryOperand(1 + argument ordinal)`, making ordinal 128 the
reachable Call maximum. The general 1023/1024 admission evidence remains in
the tail/generator suite.

### 2. Call termination is structural payload

The package cannot derive `MissingArgumentListClose` because its
`HirCallExpr` omits argument-list termination. The final parenthesized Call
payload therefore owns:

```text
HirCallArgumentListTerminator = Closed | RecoveredMissing
```

and `HirCallExpr` includes that value. It participates in structural
equality, hashing, ordering, canonical issue derivation, root poison, and retry
identity. Source spans remain solely in `HirSourceIndex`.

All Call payload, recovered component, issue, terminator, and required-token
types derive the same `Ord`/`PartialOrd` contract required by
`HirExprKind` and `HirRecoveryIssue` in addition to the package's structural
traits.

`HirCallChildStates` construction is crate-owned and validates exact shape:

- one callee state for a value callee;
- `argument_values.len() == arguments.len()`;
- type-child states equal the number and order of present-invalid or resolved
  type slots owned by the explicit application;
- every child belongs to the Call module/database and matches its payload ID.

Mismatched state slices reject before source, diagnostic, work, fact, or
project publication.

### 3. Syntax owns syntax vocabulary and attached type children

`arcweft-lang-syntax` must not depend on `arcweft-lang-hir`. The package's
syntax-facing references to `HirAssociatedCallSyntax`,
`HirCallArgumentOrdinal`, and `HirCallTypeArgumentOrdinal` are replaced by
syntax-owned enums and checked `u16` component ordinals. Final lowering maps
them directly to HIR-owned types; no alias or wrapper crosses the crate
boundary.

The pending parser projection may consume `AuthoredTypeRef` only as private
transaction input. The accepted `AttachedExpressionNode` retains revision-bound
`AttachedTypeRefNode` children keyed by syntax-owned roles:

```text
DotNominalReceiver
AssociatedReceiver
ExplicitCallTypeArgument(ordinal)
```

No `AuthoredTypeRef` range map survives as a second public/final reader.
Attachment validates source identity, child order, role continuity, and exact
component ownership before publication. HIR lowering consumes those attached
type children once and stages only final `HirSourceQuery` rows.

The attached Call projection also owns the state the package omitted:

- named `=`: `Present | Missing | InvalidPresent`;
- postfix spread ellipsis: `Present | Missing | InvalidPresent`;
- type-application terminator: `Closed | RecoveredMissing | InvalidPresent`;
- parenthesized argument terminator: `Closed | RecoveredMissing`.

These states, their central component sites, and attached expression/type
children are the complete input needed to produce the final HIR without a
detached syntax read.

### 4. Callback-block Call is preserved in the same owner

The current `CallSurfaceSyntax` also owns callback-block Calls. The package's
parenthesized-only projection cannot justify deleting that carrier. The central
`ExpressionProjection::Call` therefore distinguishes:

```text
Parenthesized(...)
CallbackBlock(...)
```

The callback branch retains the authored callee, callback closure child,
opening/closing braces, ordered parameter pattern/type children, separators,
optional fat arrow, and body component through the existing attached
expression/pattern/type vocabulary. Final HIR continues to represent the
callback as the ordinary Call argument closure already produced by current
syntax. The last `CallbackBlockCallSyntax` consumer is deleted only in the same
compiling switch that publishes this attached replacement.

### 5. Accounting rows use current shared-resolver entry semantics

The package's accounting matrix is corrected as follows:

| Row | Package value | Repository value |
|---|---:|---:|
| A-012, one argument and two semantic candidates | 3 probes | 2 probes |
| A-013, one argument and three semantic candidates | 4 probes | 3 probes |
| A-011, candidate 257 | 0 resolver invocations | 1 resolver invocation, then atomic `CandidateLimit` rollback |

Candidate probing remains `C * A`; selected multi-candidate replay remains
`A`. Proof retains at most two canonical witnesses after complete semantic
facts exist. Candidate one-over is detected inside the current shared resolver,
so entering it is one invocation even though no candidate/fact/result is
published.

### 6. The migration inventory is compiled from actual consumers

Package deletion rows naming hypothetical second Call maps or `ResolverLimits`
are not deletion evidence. The public switch is driven by actual compile
fallout and structured dependency inspection. It includes at least:

- parenthesized and callback parser emission;
- syntax source projection and attachment;
- final HIR expression/type lowering and source freeze validation;
- HIR symbol and project consumers;
- registered and non-registered checker paths;
- `CallTargetFacts`, signature focus/project, LSP projection, and Proof witness
  projection;
- runtime-plan/compiler consumers of final Call children; and
- behavior and compile-fail tests.

Obsolete `CallSurfaceSyntax`, `ArgumentListSyntax`, explicit-type surface
readers, callback surface readers, raw cursor scanners, and lossy checked-name
interpretation are deleted after all of those consumers use the final owner.
No compatibility interval is permitted.

## Implementation order

1. Finish the already-open Proof final-HIR slice so the workspace returns to a
   compiling checkpoint.
2. Add the syntax-owned central Call projection, including callback and attached
   expression/type children.
3. Replace the provisional clean-only HIR Call payload with the reachable final
   states, structural terminators, required-token states, and canonical issues.
4. Lower and freeze one complete final source manifest transactionally.
5. Connect associated classification, explicit type arguments, recovered forms,
   complete facts, accounting, signature focus, and Proof witness projection to
   the existing shared resolver.
6. Delete every detached Call/source/cursor/fact reader and repair compile
   fallout directly.
7. Run focused parse-to-query matrices, changed-crate check/Clippy, workspace
   check/strict Clippy/tests, applicable Tier 2, and structural audit before the
   coherent public switch is committed and pushed.

## Remaining boundary

There is no external design wait for E12/C01-C03 after this adjudication. The
returned ZIP is not accepted as a standalone authoritative package, but its
usable direction plus the repository decisions above are implementation-ready.
No follow-up request should be thrown for the defects listed here.
