# Coverage algorithm

## Selected owner and scope

`arcweft-lang-sema::final_analysis` owns one private
`MatchCoverageAnalyzer`. It consumes only the final checked type, pattern
facts, exact owner rows from `SCHEMAS.md`, stable pattern coordinates, and
constant guard classification. It publishes only `CheckedMatchCoverage`.
Nothing in this chapter is a compiler/runtime ABI, serialized matrix, public
pattern IR, or second type checker.

The implementation is a bounded, deterministic Maranget pattern-matrix
algorithm. The current `CoverageAtom`/`basic_coverage` approximation is
deleted in the same cut; there is no fallback path.

## Private model

```rust
struct Matrix(Vec<PatternVector>);
struct PatternVector(Vec<DeconstructedPattern>);

enum DeconstructedPattern {
    Wildcard,
    Constructor {
        constructor: CoverageConstructor,
        fields: ConstructorFields,
    },
    Or(Box<[DeconstructedPattern]>),
}

enum CoverageTypeDomain {
    Empty,
    Closed(Box<[CoverageConstructor]>),
    Product(CoverageConstructor),
    Sequence(Box<[SequencePartition]>),
    AbstractedOpen {
        observed: Box<[CoverageConstructor]>,
        other: CoverageConstructor,
    },
}

struct ConstructorFields {
    domains: Box<[CoverageTypeDomain]>,
    patterns: Box<[DeconstructedPattern]>,
}

enum SequencePartition {
    Exact(u64),
    Interval { lower: u64, upper_exclusive: Option<u64> },
}
```

These values are built lazily and memoized by semantic type digest plus the
constructor/sequence partition coordinate. They are never stored in
`FinalSemanticAnalysis`. `Error`, recovered HIR, missing exact owner rows,
ambiguous Choice injection, and type/payload mismatches reject before matrix
construction.

## Domain construction

Constructor order is semantic and total:

1. language-owned closed tag order (`Unit`; `false`, `true`; `None`, `Some`;
   `Ok`, `Err`);
2. accepted source order for project, Character, builtin, and Choice rows;
3. declaration order for product fields;
4. canonical literal bytes or accepted entity semantic ID;
5. sequence partition lower bound, with `Exact` before `Interval` at the same
   bound;
6. `Other` last.

The analyzer constructs domains as follows.

| Scrutinee domain | Coverage constructors |
|---|---|
| `Never` | `Empty` |
| Unit | one nullary `Unit` |
| Boolean | nullary `False`, `True` |
| Option | `None`; `Some(item)` |
| Result | `Ok(ok)`; `Err(error)` |
| accepted project enum | one constructor per exact checked case row, payload fields in schema order |
| Character nominal | one constructor per exact checked Character case row |
| builtin closed enum | one constructor per exact checked builtin case row |
| tuple | one product constructor, source-order element domains |
| accepted record | one product constructor, full declaration-order field domains |
| constant-length array | one product constructor with exactly `len` item fields |
| `Vec`, `Seq`, or `Slice` | the finite symbolic sequence partitions below |
| `Choice` | one source-order constructor for each inhabited alternative, carrying that alternative as one field |
| literal-capable infinite type | every canonical singleton observed in this Match plus `Other(type)` |
| Entity/open identity type | every exact accepted entity ID observed in this Match plus `Other(type)` |
| authority-declared open nominal/variant domain | observed exact constructors plus `Other(type)` |
| otherwise opaque but inhabited type | one `Other(type)` constructor |

An accepted project record is never derived from pattern field names. The
single record constructor uses the canonical checked schema/layout row; an
authored record pattern becomes that full declaration-order vector, inserting
`Wildcard` for omitted fields when rest is present. Without rest, omission is
accepted only if the language's existing pattern checker says the shape is
complete. The coverage analyzer does not revise that rule.

An infinite or open domain is exhaustive only when a useful wildcard-like row
covers `Other`. Enumerating all currently registered or observed identities
does not close it. Conversely a closed enum never receives `Other`.

### Choice

The final checker retains for every typed binding the exact set of Choice
alternative ordinals accepted under the same `TypeKind::accepts` relation and
ambiguity rule used during type checking. Coverage deconstructs that binding
as an `Or` of those alternative constructors, with a wildcard payload. A
plain binding/discard covers every alternative. A non-typed structural pattern
is admitted only with an exact, unambiguous checked alternative row; the
coverage analyzer never guesses from syntax or repeats inference.

An alternative whose recursive domain is `Empty` is omitted. If all
alternatives are empty, the Choice domain is `Empty`.

### Never

`Never` has no constructors. A Match with zero arms is exhaustive and has no
witness. Every supplied arm/Or alternative is unreachable with
`UninhabitedDomain`; no guard can make it useful.

## Symbolic sequence partitioning

Variable-length sequences are not expanded length-by-length. Before
usefulness analysis, scan all admitted sequence patterns (including nested Or
alternatives) and collect checked `u64` cut points:

- `0`;
- every exact pattern length `n` and `n.checked_add(1)`;
- every rest-pattern minimum length `n`.

Sort/deduplicate the set. Emit an `Exact(n)` partition for every authored exact
length. Cover all remaining natural lengths with disjoint half-open
`Interval { lower, upper_exclusive }` partitions between adjacent cut points
and one final unbounded interval. Empty intervals are discarded. Thus every
length belongs to exactly one partition and all patterns have uniform
acceptance within a partition.

For each partition, the constructor arity is the greatest visible prefix
length of any rest pattern that accepts it, or the exact length for an
`Exact`. Specializing a rest pattern writes its authored prefix and pads the
remaining constructor fields with wildcards. An exact pattern specializes
only its exact partition. A partition witness chooses its exact length, or the
interval's smallest `lower`; it records only the visible prefix required by
the witness.

All cut-point additions, partition appends, arity conversions, and prefix
allocations are charged before mutation. This construction is bounded by
`max_sequence_partitions`, `max_pattern_nodes`, `max_depth`, and
`max_witness_nodes`; a huge authored length never causes linear enumeration.

Constant arrays use the product rule, not symbolic sequence partitions. The
pattern checker is extended in the same cut so `TypeKind::Seq` uses the same
bracket-sequence item rule already used for Vec/Array/Slice.

## Specialization and default

Let `S(c, P)` be specialization of matrix `P` by constructor `c`:

- a row beginning with the same constructor contributes that constructor's
  field patterns followed by the row tail;
- a row beginning with `Wildcard` contributes one wildcard per constructor
  field followed by the tail;
- an `Or` head contributes each accepting alternative in source order;
- another constructor contributes nothing.

Let `D(P)` be the default matrix: rows beginning with `Wildcard` contribute
their tails; `Or` contributes each wildcard alternative's tail; constructor
rows contribute nothing. Expansion is lazy, and each emitted row is charged to
`max_matrix_rows`.

Every call to `S`, `D`, or the recursive usefulness function is charged to
`max_specializations`. Recursion depth is charged before descent. The matrix
and query are immutable values or transaction-local scratch: a limit error
cannot publish a partial answer.

## Usefulness

`useful(P, v, domains)` follows these rules:

1. If `v` and its domain vector are empty, return useful exactly when `P` has
   no empty row.
2. If the first query pattern is `Or`, test alternatives in source order.
   Each useful alternative is temporarily appended before testing later
   alternatives, so overlap is reported at the alternative's stable pattern
   coordinate.
3. If the first query pattern is constructor `c`, recurse on
   `S(c, P)`, its field patterns plus the query tail, and its field domains plus
   the domain tail.
4. If the first query is `Wildcard` and the domain is `Empty`, return not
   useful.
5. If the first query is `Wildcard` and the domain has a complete finite
   constructor set (including the finite open abstraction's `Other`), recurse
   over constructors in canonical order. The query is useful if any
   specialized query is useful.
6. A private default optimization may use `D(P)` only when the present head
   constructor signature is incomplete. It must be observationally identical
   to enumerating the domain's canonical finite abstraction and may not omit
   `Other`.

Constructor equality uses typed semantic IDs/tags and payload arity—not names,
debug strings, raw HIR IDs, or layout reconstruction. Product and sequence
specialization uses checked field/partition coordinates.

## Arm, guard, and Or ordering

Arms are processed in source order. For each arm:

1. A `ConstantFalse` guard emits one deterministic
   `ConstantFalseGuard` unreachable row and contributes nothing to the matrix.
2. Otherwise, test its pattern against the matrix of earlier unguarded or
   constant-true useful patterns. Nested Or alternatives are tested in their
   recursive source order against that matrix plus earlier useful alternatives
   from the same arm.
3. Each useless alternative emits
   `CoveredByEarlierOrAlternative` when an earlier alternative in the same arm
   covers it, otherwise `CoveredByPriorUsefulArms`. A non-Or pattern emits the
   latter at arm level.
4. `Absent` and `ConstantTrue` guards commit useful alternatives to the global
   matrix.
5. A `Dynamic` guard commits nothing. Its pattern is still checked for
   redundancy against earlier global rows; useful guarded patterns cannot
   establish exhaustiveness because the guard may be false.

The unreachable output order is arm coordinate, then depth-first Or
alternative coordinate. An arm with at least one useful alternative is not
also duplicated as an arm-level unreachable row. `max_or_alternatives` and
`max_unreachable_rows` are charged before each expansion/output append.

## Exhaustiveness and witness reconstruction

After all arms, query `useful(P, [Wildcard], [scrutinee_domain])`.

- not useful: coverage is exhaustive and `witness == None`;
- useful: coverage is non-exhaustive and the recursive call returns one
  constructor proof, reconstructed into exactly one structured witness.

At every choice point the algorithm selects the first useful constructor in
the canonical order above. Field witnesses are reconstructed in declaration
order. Open/literal residuals become `Other { type_digest }`. Sequence
intervals choose the least member. Choice witnesses retain alternative ordinal
and type digest. No source code is generated.

Witness construction charges `max_witness_nodes` before allocating or
descending. For recursive nominal types, construction is lazy: constructor
search skips a branch proven to require an already-active type without a
finite base constructor, then tries the next constructor. If no finite witness
can be established within the checked domain graph and limits, admission fails
with `UnsupportedDomain` or the exact limit error; it never claims
exhaustiveness by timeout.

## Budget transaction

One transaction-local counter set owns:

```text
arms, matrix_rows, or_alternatives, pattern_nodes, expression_nodes,
depth, sequence_partitions, specializations, unreachable_rows,
witness_nodes, transcript_bytes
```

Every counter and configured maximum is `u64`. The operation is always:

```text
attempted = current.checked_add(delta) else ArithmeticOverflow(kind)
if attempted > limit: LimitExceeded(kind, limit, attempted)
current = attempted
perform allocation/descent/write
```

No `saturating_add`, lossy cast, unchecked multiplication, `expect`, or
post-allocation check is permitted. Multiplication uses `checked_mul`; byte
lengths use `u64::try_from`. Exact-limit cases pass, every one-over case fails,
and repeated runs return the same error coordinate/counter.

## Required differential properties

Implementation tests compare the private optimized algorithm against a small
independent exhaustive oracle only for bounded finite generators. Required
properties include:

- row permutation changes diagnostics as source order requires but never the
  final covered value set for all-true guards;
- Or flattening preserves coverage and source-order redundancy coordinates;
- adding a useful unguarded row cannot make a previously covered witness
  reappear;
- replacing a dynamic guard with true can only increase coverage;
- literal/entity enumeration without wildcard never covers `Other`;
- record field source order does not affect product semantics, while declared
  field order determines witness layout;
- symbolic sequence results equal enumeration for generated lengths within the
  oracle bound;
- all exact-limit and one-over cases are atomic and deterministic;
- `Never` and Choice-with-only-Never-alternatives obey the empty-domain rules.
