# Proof 01.1.1.4.1.1.1.1.2 Call recovery return intake

Date: 2026-07-29

Status: `RETURNED_REJECTED_NOT_READY_FOR_IMPLEMENTATION`

## Archive identity and mechanical validation

The externally returned archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2-call-recovered-argument-schema-correction-final-contract.zip
```

- byte length: `43,184`;
- SHA-256:
  `BC8DE35E8C4D69008344EC44B9CFF1C5C59EE17ECB2CA54006B0ECF6EE923B50`;
- baseline: `f5087621eb764d421f95c99ee05eaae3c5f2f4d2`, equal to the
  audited `main`;
- `29` unique safe members;
- `28` intentional non-manifest rows, all with exact byte length and SHA-256;
- `REQUEST_COPY.md`: byte-identical to the repository request, SHA-256
  `BAD96A39E3B5CB671C0C4FA80EA7D8082F089FCF528AB0A8BA465CE38ED15578`;
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION` plus newline;
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`; and
- `TEST_MATRIX.tsv`: `111` well-formed rows with no duplicate test ID.

The archive is mechanically valid. It is not copied into Git. This path and
digest are the retained identity of the rejected return.

## Adjudication

The repository rejects the archive's self-status. It is not
implementation-ready because its proposed owner and observable behavior
conflict with accepted contracts and the current language.

### Required predecessor evidence was not read

The archive records `predecessor_archives_direct=0/3`.
`PREDECESSOR_AUDIT_SUMMARY.md` marks the tail, source-owner, and leaf archives
`NOT_DOWNLOADED`; `PREDECESSOR_MEMBER_AUDIT.tsv` contains only its header; and
all six rows in `REFERENCED_REPOSITORY_MATERIALS.tsv` are
`NOT_DIRECTLY_AVAILABLE`.

The required repository archives do exist and were opened during this intake:

| Authority | SHA-256 |
|---|---|
| Proof 01.1.1.4.1 leaf | `61E2EE166BFF158FE83DCF1484B7B9380A81F60D865377503400D27D238CC708` |
| Proof 01.1.1.4.1.1 source owner | `2BCD3F78EFB76442C2698A24251C4D874F7A941C5A8985649EA157100908A72E` |
| Proof 01.1.1.4.1.1.1.1 tail/generators | `69DC42FC7C985FED638D08D694ED301291A50AF3CEFA7117321D4219BE7E6471` |

The semantic drift below is consistent with that missing audit.

### It creates a second source authority

The accepted source contract has one component map and one public query:

```text
HirSourceIndex.components: BTreeMap<HirSourceQuery, HirSourceSite>
HirModule::source_site(expected_source, query) -> HirSourceLookup
```

`Whole` remains arena-slot metadata. `AbsentOptional` is a lookup presence,
owner poison is `HirSourceOwnerStatus`, and an inapplicable role is a typed
query error.

The return instead defines `HirCallSourceSurface.components`,
`HirCallSourceSurface::component`, `HirSourceIndex::call_surface`, a second
`Whole`, and stored `AbsentOptional` / `RoleNotApplicable` values. It also uses
raw revision/range vocabulary without the accepted complete
`SourceDocumentIdentity`, `SourceSpan`, checked insertion, and retained-length
validation. This is a parallel source map and reader, not an E12 extension of
the accepted authority.

### It specifies the wrong Arcweft syntax

The normative grammar is:

```text
CallArg := Expr | Ident '=' Expr | Expr '...'
```

The returned tests use `f(name: x)` and `f(...xs)`, and its source matrix says
that a named-argument colon belongs to `Whole`. Current Arcweft uses
`f(name = x)` and postfix spread `f(xs...)`. The returned matrix therefore does
not exercise the current Call family.

### Signature-focus boundaries change accepted behavior

The current typed argument-list authority admits cursors from `open.end()`
through the close start or missing-close recovery end. The opening token itself
is outside. A comma belongs to the following slot from its start. A trailing
comma selects the one-past slot from its start. Without a trailing comma, the
final argument remains active at the close start.

The return instead makes a separator start select the preceding slot, makes
the opening delimiter select slot zero, and makes every closing delimiter
select one-past. It also relabels the requested AW-AH R05, R09, and R14 rows
rather than restating their accepted fixtures.

### Call poison is not representable by the returned payload

`HirCallExpr.issues` is declared to be derived only from the callee, explicit
type application, and arguments. However:

- `Missing { recovery }` cannot distinguish `MissingCallee` from
  `UnresolvedDotMember`;
- `AssociatedType { receiver: TypeId, member }` has no receiver recovery,
  nominal terminal-error, separator-family, or generic-arity state;
- `HirCallIssue` drops accepted `InvalidAssociatedReceiver` and
  `BareGenericArity`; and
- the accepted `HirAssociatedCallSyntax::{DotFallback,
  ExplicitDoubleColon}` has no final replacement.

The return also retains multiple issues in a boxed slice while the accepted
root poison contains one `HirRecoveryIssue::InvalidCall(HirCallIssue)`. It does
not define the replacement root-poison schema or the variant-by-variant
canonical issue key table. Consequently payload equality, retry identity,
owner poison, and diagnostic ordering are not determined.

An invalid-present explicit type argument is represented as `Invalid` without
a `TypeId`, even though final typed lowering allocates a real qualified
poisoned `TypeId` for present invalid type syntax. This loses a semantic child
and its identity. Only a genuinely missing type slot may have no authored
`TypeId`.

### It replaces rather than integrates the shared resolver

The return invents `SharedCallableResolver::resolve_checked_call`, a reduced
`CheckedCallFact`, and `ResolverLimits { 2, 2 }`. The accepted production
authority is the existing shared `resolve_call_target` / checked-call pipeline,
its complete `CallTargetFacts`, and `CallableLimits`, including the operational
candidate ceiling of `256`.

The proposed fact drops existing target/result/effects, curried groups,
function-value type, mapped slots, inferred/expected types, and detailed poison
state. Signature projection cannot derive a selected candidate from the
returned fact alone.

Accounting is also contradictory. The matrices say each authored/recovered
argument is checked exactly once, while the constructor order says it is
checked once per admitted candidate. AW-AH-009.3.3.4 additionally requires a
bare-generic/type failure to invoke the shared resolver zero times while still
checking every retained argument once in recovery. The return instead invokes
the resolver for every poison Call and contains no physical-probe versus
retained-fact accounting matrix.

### Attached replacement and exact limits are incomplete

Only an attached callee target is defined. No attached owner carries the
ordered current-grammar arguments, recovered names/values, punctuation,
explicit type arguments, delimiters, separators, trailing separators, or
recovery end. Deleting `ArgumentListSyntax` under that schema would lose the
geometry; retaining it would leave a dual reader.

The return duplicates accepted identity and limit owners with
`HirCallArgumentIndex`, `CallLoweringLimits`, and `ResolverLimits` instead of
reconciling `HirCallArgumentOrdinal`, central `HirLimit`, and
`CallableLimits`. It also omits the accepted RichText call limit of `32`.

The Call producer can emit RecoveryOperand ordinals only `0..=128`, but
`T-RB-12-013/014` attempt to reach `1023/1024` through a full Call pipeline.
The 128-argument preflight makes those tests unreachable. The general
1023/1024 role-admission evidence belongs to the accepted generator contract;
E12 must test its reachable ordinal boundary.

## Usable direction

The following decisions remain useful but are not sufficient for acceptance:

- known Call syntax remains `HirExprKind::Call` with typed poison;
- every authored argument slot remains in source order and retains its
  positional/named/spread form;
- missing/invalid argument names are typed states and never fabricated names;
- missing callee/value expressions use root-owned `RecoveryOperand` keys at
  ordinal zero and `1 + argument ordinal`;
- dot-member evidence remains attached until value-first/nominal-second
  classification is final;
- explicit call type applications need one ordered qualified final owner; and
- migration remains deletion-driven with no compatibility reader.

## Follow-up and implementation boundary

The standalone redelivery request is
[Proof 01.1.1.4.1.1.1.1.2.1 Call source/resolver authority correction](../reviews/requests/2026-07-29-seq-proof-01.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction.md).

E12/C01-C03 and their public Call authority switch remain design-blocked until
that return is accepted. The obsolete static Capacity string helper and early
success path remain deleted and must not be restored. E14-E35 and other
decision-complete private final-HIR work remain independently implementable.

