# Generic Match, bounded coverage, and separate retained View admission

## 1. Two semantic products

The compiler publishes two distinct products:

```text
generic language product
  CheckedMatch
    - checked HIR child references
    - normalized types
    - patterns/bindings/guards/bodies
    - bounded coverage and reachability
    - CheckedMatchSemanticDigest

retained View product
  CheckedViewMatchAdmission
    - CheckedMatchRef
    - retained output/capture coordinates
    - Copy/SnapshotClone dispositions
    - exact consulted ownership evidence
    - CheckedNeedProducerAdmission
    - CheckedViewMatchAdmissionDigest
```

The second consumes the first. The first never calls, stores, or depends on the
second. Therefore an ordinary Match that moves/destructures an affine value can
be valid even when retaining that value in a View is rejected.

## 2. Generic `CheckedMatch::try_from_hir`

Construction order is exact:

1. resolve the owner `ExprId` in the accepted HIR generation;
2. require an exact HIR Match expression and read its scrutinee/arms directly;
3. enforce arm count and checked-node work limits before allocation;
4. resolve the checked scrutinee expression and normalized type;
5. for each arm in source order:
   - validate scope, pattern, optional guard, body, and locals;
   - validate checked pattern type against scrutinee type;
   - derive stable pattern binding coordinates;
   - validate every binding's checked type;
   - require every present guard type to be Boolean;
   - classify the guard only from its checked expression resolution;
6. invoke the private `MatchCoverageAnalyzer`;
7. reject non-exhaustiveness;
8. retain sorted unreachable-arm evidence;
9. compute `CheckedMatchSemanticDigest`;
10. publish one complete `CheckedMatch`.

No caller argument contains `coverage`, `exhaustive`, `unreachable`, ownership,
View site, producer admission, resource registry, or accepted revision.

All HIR IDs retained in the object are generation-local lookup coordinates and
are excluded from its semantic digest and product identity.

## 3. Exact guard classification

```rust
match checked_guard.resolution() {
    CheckedExpressionResolution::Literal(HirLiteral::Boolean(true)) =>
        CheckedGuardClass::ConstantTrue,
    CheckedExpressionResolution::Literal(HirLiteral::Boolean(false)) =>
        CheckedGuardClass::ConstantFalse,
    _ =>
        CheckedGuardClass::Dynamic,
}
```

This rule is closed even when another expression is semantically pure or could
be folded. Until a separate checked constant authority is accepted, these are
Dynamic:

- a Boolean local initialized from a literal;
- unary `!false`;
- `1 == 1`;
- a const function call;
- a source string spelling `"true"`;
- an enum/nominal constant;
- a runtime-plan constant;
- any poisoned/recovery expression, which is rejected earlier.

No source interpreter, string fold, or evaluator is called.

## 4. Guard/coverage truth table

| Guard | Arm may execute | Contributes to exhaustiveness | Covers later rows | Unreachable reason precedence |
|---|---:|---:|---:|---|
| absent | yes | yes | yes | pattern usefulness |
| `ConstantTrue` | yes | yes | yes | pattern usefulness |
| `ConstantFalse` | no | no | no | `FalseGuard` first |
| `Dynamic` | yes | no | no | pattern usefulness against prior contributing rows |

A `ConstantFalse` arm is retained as unreachable with `FalseGuard` even if its
pattern is also covered by prior rows. This precedence is independent and
deterministic.

A Dynamic guarded wildcard does not make the Match exhaustive. A later
unguarded wildcard remains reachable and is required to close an open domain.

## 5. Bounded Maranget owner

`MatchCoverageAnalyzer` is crate-private and constructs
`CheckedMatchCoverage`. It implements the typed Maranget usefulness algorithm
with lazy Or expansion, symbolic infinite-domain residuals, and witness
generation.

Algorithm:

1. normalize each checked pattern into a lazy vector;
2. obtain the exact constructor domain from checked type/accepted nominal facts;
3. test each arm's pattern usefulness against prior contributing rows;
4. assign `FalseGuard` immediately for ConstantFalse;
5. retain unreachable evidence in source order;
6. add only absent/ConstantTrue rows to the exhaustiveness matrix;
7. specialize a wildcard witness after all arms;
8. reject if a witness remains;
9. sort retained unreachable rows by arm ordinal and reject internal duplicate
   evidence;
10. return the private complete coverage value.

### 5.1 Pattern matrix

| Pattern family | Coverage representation | Rule |
|---|---|---|
| discard | wildcard | covers the admitted domain |
| binding/mutable binding | wildcard | binding does not narrow |
| whole binding | child | delegates to nested pattern |
| typed binding | type intersection | exact checked type; mismatch failed earlier |
| literal | singleton + Other | canonical literal bits/bytes |
| entity reference | singleton + open residual | accepted stable entity identity |
| closed variant | finite constructor set | accepted declaration/case order |
| tuple | one product constructor | exact arity |
| record | declaration-order product | omitted rest fields are wildcard |
| constant array | fixed product | concrete length only |
| Vec/Slice/Seq exact | exact symbolic length | exact prefix without rest |
| Vec/Slice/Seq rest | interval `[prefix, infinity)` | rest binding does not narrow |
| Or | lazy source-order union | expand only selected specialization |
| open opaque/future domain | open residual | only total wildcard closes |
| poisoned/unsupported decomposition | hard error | never enters matrix |

Additional rules:

- Unit has one constructor.
- Never has none; zero arms is exhaustive and supplied arms are unreachable.
- Bool is exactly false/true.
- Result/Option use their accepted semantic case order.
- Choice uses checked source-order alternative identities.
- Project/builtin variants use accepted declaration order.
- Records use accepted declaration field order, not map iteration.
- Arrays with generic/inferred/poisoned length fail coverage publication.
- Infinite scalar domains do not enumerate; explicit singleton constructors
  coexist with `Other`.
- Open/future-nonexhaustive/opaque domains retain `OpenResidual`.
- Or alternatives must already have the same checked binding/type shape.

## 6. Coverage limits

```text
max_arms                 = 4_096
max_matrix_rows          = 8_192
max_or_alternatives      = 4_096
max_pattern_nodes        = 65_536
max_recursion_depth      = 64
max_sequence_partitions  = 2_048
max_specializations      = 32_768
max_unreachable_rows     = 4_096
max_witness_nodes        = 1_024
```

Each counter is checked `u64`, charged before allocation or descent.
Exact-limit input may succeed. One-over fails with no CheckedMatch, digest,
unreachable diagnostics, or View row.

Diagnostic order inside generic Match:

```text
stale/malformed HIR or poison
< missing checked child / type mismatch / non-Boolean guard
< missing or unsupported constructor owner
< work limit
< non-exhaustive witness
< retained unreachable warnings
```

## 7. Stable generic Match digest

The exact transcript is in `IDENTITY_AND_DIGESTS.md`. Stability properties:

- rebuilding identical checked semantics in a different HIR arena gives the
  same digest;
- changing only SourceSpan or debug spelling gives the same digest;
- changing pattern structure, binding coordinate/type, literal guard class,
  body semantics, exhaustive value, or unreachable reason changes the digest;
- View program/site/revision/admission cannot affect the digest.

The private checked expression and pattern encoders are exhaustive inherent
methods on their original checked enums. No source AST serializer or generic
Serde is accepted.

## 8. Need producer admission

`CheckedNeedProducerAdmission` is constructed before View admission from exact
producer argument/capture values.

For each retained producer value it records:

- a stable checked value coordinate;
- exact semantic type digest;
- `Copy` or `SnapshotClone`;
- consulted ownership evidence row(s); and
- an exact value-level certificate where type alone is insufficient.

Construction is transactional. A rejected value yields no partial certificate
or digest. The digest does not contain producer contract identity, task plan,
runtime values, policy, generation, or task identities.

The runtime producer contract is built later from accepted callable/host
catalog facts. Admission cannot mint or select a producer contract.

## 9. `CheckedViewMatchAdmission`

Inputs:

```text
CheckedMatchRef
exact source-order retained outputs
exact source-order retained captures
CheckedNeedProducerAdmission
CheckedOwnershipContext { ProjectSymbolTable, RegisteredSemanticWorld }
CheckedOwnershipLimits
```

Construction:

1. resolve `CheckedMatchRef` in the exact accepted generation;
2. validate output/capture coordinates and reject duplicates;
3. classify outputs in source order;
4. classify captures in source order;
5. merge/deduplicate exact consulted evidence for digest only;
6. validate producer-admission Match/payload relationship;
7. compute `CheckedViewMatchAdmissionDigest`;
8. publish the complete admission.

A failure blocks only the corresponding checked View catalog/product row.
Generic Match facts and ordinary lowering remain published.

### 9.1 Admission matrix

| Value use | Generic Match | View retained output/capture | Need producer argument/capture |
|---|---:|---:|---:|
| scalar Copy value | accept | Copy | Copy |
| owned snapshot value | accept | SnapshotClone | SnapshotClone |
| affine value moved/destructured | accept if ordinary type rules permit | reject | reject |
| Stream | accept ordinary matching where language permits | reject | reject |
| borrow/frame local | accept ordinary lexical use where language permits | reject | reject |
| Function type | accept ordinary use | reject without exact value certificate | reject without exact value certificate |
| `ViewValue` | accept ordinary type checking | reject MissingViewPersistenceEvidence | reject |
| `Need<T>` handle | accept | SnapshotClone with producer certificate | SnapshotClone with producer certificate |
| opaque Plain | accept | SnapshotClone | SnapshotClone |
| opaque AffineHandle | accept ordinary lexical use | reject | reject |

## 10. Stable View site

`ViewMatchSiteId` is derived from:

1. current `ViewProgramId`;
2. accepted enclosing declaration semantic identity; and
3. the closed checked-expression child-role path.

Source-order child indexes are semantic structure, not HIR allocation. The
path includes roles such as Match arm body/guard and call argument ordinal; it
never includes HIR IDs or byte spans.

`CheckedViewMatchCoordinate` is:

```text
(program: ViewProgramId, site: ViewMatchSiteId,
 admission: CheckedViewMatchAdmissionDigest)
```

The coordinate does not carry `CheckedMatchSemanticDigest` separately because
admission already commits it.

## 11. View catalog and bundle row

```rust
pub struct CheckedViewMatchCatalogRow {
    pub coordinate: CheckedViewMatchCoordinate,
    pub checked_match: CheckedMatchRef,
    pub checked_match_digest: CheckedMatchSemanticDigest,
    pub need_admission: CheckedNeedProducerAdmissionDigest,
    pub ownership_evidence: OwnershipEvidenceDigest,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub producer_contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub arguments: RuntimeValueDigest,
    pub resource_dependency: Option<ResourceDependencyDigest>,
}

pub struct AcceptedViewMatchBundleRowV1 {
    pub version: u8, // exactly 1
    pub program: ViewProgramId,
    pub accepted_revision: AcceptedViewProgramRevision,
    pub match_row: CheckedViewMatchCatalogRow,
}
```

The bundle codec validates revision through the current
`AcceptedViewProgramRevision` owner. It validates the coordinate/digest joins
against compiler/AWBC products. It neither hashes revision into producer
identity nor constructs an alternative View digest.

For current Agent DTO resource rows,
`resource_dependency` is `None`; there is no unkeyed registry digest. A future
exact resource-bearing type may provide a typed dependency without changing
current rows.

## 12. Runtime View evaluation

At mount/subscription time runtime-driver:

1. resolves the accepted bundle row by `CheckedViewMatchCoordinate`;
2. validates active accepted revision against the bundle registry;
3. resolves the checked producer template and arguments;
4. recomputes `NeedProducerInstance`;
5. obtains or launches the Join handle through the sole task boundary;
6. registers the existing typed View observer key;
7. reads the correlated RuntimeNeedState;
8. projects NotStarted/Pending/Ready/Cancelled into the ordinary checked Match
   selector input;
9. executes the accepted Variant/Tuple selector ABI with explicit guard Branch
   lowering;
10. installs selected arm bindings transactionally;
11. queues one bounded View invalidation.

No View VM, ViewRuntimeValue, public runtime-driver Match result type, escaped
AWBC register, or copied coverage table is introduced.

## 13. Revision and replacement

The accepted revision is used for:

- catalog row validation;
- bundle cross-section validation;
- registry publication;
- hot-replacement old/new transaction identity; and
- rejecting stale catalog products.

It is not used for:

- generic Match digest;
- View admission digest;
- View site;
- task plan digest;
- producer contract/instance;
- NeedId/TaskKey/TaskId; or
- runtime argument digest.

A revision-only replacement with identical explicit site mapping and all
rebind evidence retains live state. A revision mismatch by itself is expected,
not a cancellation reason.

## 14. Failure precedence across products

```text
generic HIR/type/pattern/guard failure
< generic coverage limit/non-exhaustiveness
< generic Match digest construction
< producer argument/capture ownership
< retained View output/capture ownership
< stable View site/coordinate construction
< compiler/AWBC product join
< strict bundle decode/revision validation
< runtime producer/correlation validation
< publication cursor/state transition
< View selector result/binding installation
```

Each boundary publishes only a complete product. Later failure never deletes or
invalidates a valid generic Match fact unless its accepted semantic generation
itself is replaced.
