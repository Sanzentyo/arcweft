# Inherent checked Match coverage authority

## Sole construction path

`CheckedMatchCoverage` has private fields and no public or caller-facing
constructor. The only constructor is `MatchCoverageAnalyzer::analyze`, a
crate-private owner inside `arcweft-lang-sema::final_analysis`.

```rust
impl CheckedMatch {
    pub(crate) fn try_from_hir(
        module: &HirModule,
        owner: ExprId,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        patterns: &BTreeMap<PatternId, CheckedPattern>,
        bindings: &BTreeMap<LocalId, CheckedBinding>,
        symbols: &ProjectSymbolTable,
        catalogs: FinalSemanticCatalogs<'_>,
        limits: CheckedMatchLimits,
    ) -> Result<Self, CheckedMatchConstructionError>;
}
```

The constructor reads the accepted HIR arm list directly, validates all child
facts and guard Bool types, invokes coverage and ownership, and publishes only
a complete `CheckedMatch`. A call signature containing `coverage`,
`exhaustive`, or `unreachable_arms` is forbidden.

## Algorithm

The analyzer implements the typed Maranget usefulness algorithm with a
constructor matrix and witness generation.

1. Normalize one checked pattern into a lazy pattern vector. Or-patterns retain
   source order and expand only when the selected column is specialized.
2. Determine the scrutinee domain from checked `TypeKind`, exact project
   nominal facts, registered accepted nominal facts, and closed builtin owners.
3. For each arm in source order, classify its guard as Absent, ConstantTrue,
   ConstantFalse, or Dynamic using the existing checked constant-expression
   result. No source text is evaluated.
4. Test arm usefulness against prior contributing rows. An arm is unreachable
   when every value it can match is already covered by prior Absent or
   ConstantTrue rows. ConstantFalse is also unreachable with reason
   `FalseGuard`.
5. Add the row to the coverage matrix only for Absent/ConstantTrue.
6. After all arms, specialize the wildcard witness against the matrix. A
   surviving witness is a non-exhaustive hard error.
7. Sort unreachable rows by arm ordinal, reject duplicate internal rows, and
   publish diagnostics only after the complete fact and digest inputs validate.

The matrix does not enumerate infinite primitive values or all sequence
lengths. Infinite scalar domains use explicit singleton constructors plus an
`Other` constructor. Sequence patterns create disjoint symbolic length
partitions from observed exact lengths and rest lower bounds.

## Pattern-family rules

| Pattern family | Coverage model | Exact rule |
|---|---|---|
| `discard` | `wildcard` | covers whole admitted domain |
| `binding` | `wildcard` | binding/mutable binding do not narrow |
| `whole_binding` | `child` | whole binding delegates to child |
| `typed_binding` | `type_intersection` | checked exact type; mismatch is earlier hard error |
| `literal` | `singleton_plus_other` | canonical literal bits/bytes; infinite domains retain Other |
| `entity_reference` | `singleton_plus_other` | stable checked entity identity; open residual remains |
| `closed_variant` | `finite_constructor_set` | project enum, builtin closed enum, Result, Option, Choice |
| `tuple` | `product` | single arity constructor with nested columns |
| `record` | `product` | declaration-order fields; omitted rest fields become wildcards |
| `array` | `fixed_product` | constant length only; unresolved length is hard error |
| `vec_slice_seq_exact` | `length_partition` | exact prefix length without rest |
| `vec_slice_seq_rest` | `length_interval` | [prefix, infinity), rest binding does not narrow |
| `or` | `ordered_union` | lazy union; duplicate alternatives do not fabricate arm reachability |
| `open_opaque_future` | `residual_open` | only wildcard can close; unsupported decomposition is hard error |
| `poisoned` | `hard_error` | never enters coverage matrix |

Additional closed rules:

- `Unit` has one constructor. `Never` has none; zero arms are exhaustive and
  every supplied arm is unreachable.
- Bool is the finite set false/true.
- Result and Option use their semantic case order. Anonymous Choice uses the
  checked source-order alternative identities. Project/builtin variants use
  their accepted closed declaration order.
- Tuples are one product constructor with exact arity.
- Project records use declaration-order fields. Exact records must already pass
  field completeness. Ignore-rest and whole-record binding fill omitted fields
  with wildcard columns.
- Constant arrays require an accepted concrete length. Generic, poisoned, or
  inferred lengths are hard errors for coverage publication.
- Vec/Slice/Seq exact patterns cover exactly their prefix length. A tail rest
  covers `[prefix_len, infinity)`. The rest binding itself does not narrow.
- Or alternatives must already agree on binding/type shape. Redundant
  alternatives may produce a local diagnostic note, but the public
  unreachable-arm evidence remains arm-based and cannot be forged.
- Open/future-non-exhaustive/opaque domains retain an `OpenResidual`; only a
  total wildcard closes it. Constructor decomposition without an owner is a
  hard error.

## Guard semantics

| Guard class | Arm may execute | Contributes to exhaustiveness | Covers later arms |
|---|---:|---:|---:|
| Absent | yes | yes | yes |
| ConstantTrue | yes | yes | yes |
| ConstantFalse | no | no | no |
| Dynamic | yes | no | no |

A dynamic guarded wildcard therefore does not make the Match exhaustive. A
later wildcard remains reachable. A dynamic guarded arm whose pattern was
already fully covered is unreachable independently of its runtime guard.

## Publication and diagnostics

Hard failures: stale/missing HIR fact, non-Bool guard, poisoned pattern/domain,
missing closed constructor owner, unsupported decomposition, limit overflow,
and non-exhaustiveness. Any hard failure publishes no CheckedMatch, no
unreachable warnings, no View row, and no digest.

Unreachable arms are warnings plus retained evidence after success. Primary
source roles are the arm pattern, or the guard for `FalseGuard`; the pattern is
secondary in the latter case. Non-exhaustiveness uses the whole Match as
primary, scrutinee and final arm as secondary, and a structurally formatted
witness that never becomes identity.

Diagnostic precedence is:

```text
HIR structure/poison
< missing checked child or type mismatch
< missing/unsupported domain owner
< work limit
< non-exhaustive
< unreachable warnings
```

## Bounded work

```text
{
  "max_arms": 4096,
  "max_matrix_rows": 8192,
  "max_or_alternatives": 4096,
  "max_pattern_nodes": 65536,
  "max_recursion_depth": 64,
  "max_sequence_partitions": 2048,
  "max_specializations": 32768,
  "max_unreachable_rows": 4096,
  "max_witness_nodes": 1024
}
```

Every counter is a checked u64 charged before allocation or recursion.
Exact-limit input can succeed; one-over is a hard error with no partial result.
Traversal and diagnostics are deterministic in source/declaration order.
