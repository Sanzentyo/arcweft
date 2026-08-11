# Shared resolver, facts, and accounting integration

## Existing authority retained

The implementation continues to enter the existing checked-call pipeline and
`resolve_call_target`. It does not add `resolve_checked_call`, a reduced fact,
a Capacity-only dispatcher, or a parallel candidate list.

The pre-resolver prepared call contains:

- final Call root and exact expected source identity;
- classified callee;
- ordered explicit call `TypeId` arguments;
- ordered retained argument forms/names/value expressions;
- candidate-neutral inferred argument facts;
- central source-query coordinates;
- current/next group context.

Explicit call type arguments are supplied exactly once to generic
instantiation. Associated-receiver arguments remain in the receiver `TypeId`.

## Existing observable facts retained

`CallTargetFacts` continues to expose:

- selected, ambiguous, rejected, non-callable, or missing target;
- selected and considered candidate identity;
- result type and effect row;
- current and next curried group;
- function-value type;
- every retained authored/recovery argument;
- every typed slot and mapped parameter;
- inferred and expected types;
- call/slot poison and diagnostics;
- focused active parameter.

Only the lossy optional-name/spread combination is replaced by
`CheckedCallArgumentForm` and `CheckedCallArgumentName`.

## Exact accounting axes

The existing `ResolverWork`/`SignatureWorkReport` owners gain counters; no
parallel work owner is created.

| counter | meaning |
|---|---|
| `logical_argument_checks` | each retained authored/recovery argument admitted and expression-checked once independent of candidate count |
| `resolver_invocations` | entry into existing shared target resolver |
| `candidate_argument_probes` | physical candidate/argument compatibility probes |
| `selected_replay_argument_visits` | winning multi-candidate transaction replay visits |
| `retained_argument_fact_publications` | argument facts committed to `CallTargetFacts` |
| `signature_argument_projections` | argument coordinates projected by one signature query |
| existing resolver/mapping/type-check work | still charged under `CallableLimits.max_query_work()` |

"Checked once" means `logical_argument_checks == retained_argument_count`.
It does not mean physical probe count is one.

## Path formulas

Let `A` be retained arguments and `C` admitted candidates.

- associated receiver/member/arity terminal failure:
  `logical=A`, resolver `0`, probes `0`, replay `0`,
  publications `A`;
- singleton selected/rejected:
  `logical=A`, resolver `1`, probes `A`, replay `0`,
  publications `A`;
- multi-candidate selected:
  `logical=A`, resolver `1`, probes `C*A`, replay `A`,
  publications `A`;
- multi-candidate ambiguous/rejected:
  `logical=A`, resolver `1`, probes `C*A`, replay `0`,
  publications `A`.

A rejected singleton may retain its already checked probe evidence without a
second evaluation.

## Associated path

1. dot syntax performs value lookup;
2. value present or terminal value error owns the call;
3. only definitive value absence permits nominal resolution;
4. explicit `::` enters nominal-only resolution;
5. project/declaration metadata resolves nominal identity and declared generic
   arity;
6. bare-generic arity failure produces a poisoned associated receiver, checks
   all retained arguments once, and performs zero shared-resolver invocations;
7. otherwise one existing shared resolver invocation applies precedence:
   typed environment method, Capacity, associated trait.

## Proof two-witness reconciliation

Production `CallableLimits.max_candidates_per_call()` remains 256.
`CallTargetFacts` keeps the complete bounded considered set.

The Proof verifier projection is not a resolver and does not select candidates.
It retains at most two canonical candidate/result witnesses for deterministic
proof payload size:

1. primary selected/focused/best-rejected witness;
2. first distinct ambiguity/conflict witness.

It also retains exact `considered_count` and `omitted_count`. A third semantic
candidate/result is admitted by sema, represented in complete `CallTargetFacts`,
and increments `omitted_count`; it does not cause a two-candidate resolver limit
and does not erase semantic facts. Exact `2`/`3` rows therefore test Proof
projection/truncation accounting, while resolver exact/one-over is `256/257`.
