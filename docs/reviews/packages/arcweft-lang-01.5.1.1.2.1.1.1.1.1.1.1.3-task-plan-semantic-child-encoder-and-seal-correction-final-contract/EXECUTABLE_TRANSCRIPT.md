# Structured RuntimePlan executable transcript

## 1. Fixed table inventory

The executable encoder visits exactly fifteen table families. Tables `0..13`
write an owner-produced row digest; table `14` writes the task-plan base row
inline. Every list is canonical dense/source order, never hash-map iteration
order.

| Tag | RuntimePlan owner | Row order committed | Included semantic roles in owner row digest | Explicitly excluded |
|---:|---|---|---|---|
| 0 | runtime type table | dense type ordinal | closed runtime type tag; child type ordinals in type-defined order; accepted nominal identity/layout; opaque producer, class, persistence, and type arguments | source type spelling, HIR type ID, debug name |
| 1 | local declarations | declaration/source ordinal | semantic type ordinal; storage/mutability role; initialization role; owner function/flow coordinate | local spelling, span, slot address |
| 2 | nominal record domains | accepted domain/source order | owner nominal semantic identity; fields in declaration order with field ordinal, accepted field identity, and type ordinal; layout role | field source spelling and source offsets |
| 3 | variant domains | accepted domain/source order | owner nominal semantic identity; cases in declaration order with case ordinal, accepted case identity, and optional payload type | case spelling, source offsets |
| 4 | function sites | function semantic order | accepted function semantic identity; function role; parameter order/modes/types; canonical captures; return type; body root; endpoint order/roles | task-plan key/digest, source name, HIR ID, compiled address |
| 5 | dialogue content plans | content/source order | content kind; parts, marks, value sites, line effects, and child content references in source order; typed expression/flow roots; line group coordinate | source ranges, localization/debug text not used as semantic ID |
| 6 | entries | accepted entry order | entry kind; accepted public/capability identity; parameter and result ABI roles; callable/flow/function target; capability requirements in declaration order | launch profile debug labels and source path |
| 7 | callable executables | accepted callable order | runtime callable identity and accepted contract; receiver mode; parameter ABI order; result ABI; function/body target and effect contract reference | source callable spelling, JIT/AOT choice |
| 8 | flow executables | accepted flow order | flow semantic identity; parameter/result types; checked effect/control contract; body flow coordinate | source name, code address |
| 9 | flows | flow/source order | operation tree tags; child operation roles in source order; typed locals/expressions/patterns; branch/loop/match role order; task-plan construction coordinates | completed task-plan digest, HIR IDs, spans |
| 10 | pure helpers | helper/source order | accepted helper identity; parameter/capture order; result type; typed body root | debug/helper source spelling |
| 11 | trait methods | accepted method order | accepted trait/method identity; receiver mode; parameter order; result/effect contract; implementation body coordinate | method source spelling and dispatch cache |
| 12 | line task groups | group/source order | group semantic identity; line nodes, triggers, effects, cancel rules, and child node roles in source order; `LinePlanSemanticDigest`; task-plan build coordinates | final task-plan keys/digests, debug line labels |
| 13 | stream plans | stream/source order | stream operation tags; input/output types; transform/filter/match child roles; arms and bindings in source order; suspension/effect contract references | source stream spelling, HIR IDs, runtime cursor |
| 14 | `RuntimeTaskPlan` candidates | task declaration/source order | exact inline base fields from `TRANSCRIPTS.md` section 7 | completed key/self/expected digest and upper View payload |

## 2. Source-order roles

A source-order role is encoded as a zero-based `u32-le` ordinal immediately
before the row payload. Child order inside a row is likewise encoded by each
owner's exhaustive semantic visitor. The following roles are never normalized
by sorting:

- parameters, arguments, captures where the accepted capture owner defines
  canonical capture order;
- record fields and variant cases in accepted declaration order;
- expression, flow, pattern, dialogue, line, and stream children in their
  closed owner-defined order;
- Match arms and bindings in their accepted semantic order;
- task plans in runtime-plan lowering order; and
- effect rows and child contract references in checked declaration order.

Only collections whose accepted semantic owner already defines a canonical key
order may be sorted. Such a sort is performed by that owner before the row
visitor starts; the executable encoder never sorts an arbitrary map.

## 3. Task references before sealing

A typed task launch in a function, flow, dialogue, line, or stream row stores a
`RuntimeTaskPlanBuildCoordinate`, not a digest. Its semantic row transcript
writes:

```text
task_reference_tag:u8
coordinate_ordinal:u32-le
```

The private coordinate owner token is validated in memory but is not hashed.
The ordinal must resolve to the same candidate and match table-14 source order.
This representation is necessary for acyclic hashing and is not a public
runtime identity.

After sealing, the immutable RuntimePlan may resolve a coordinate/index to the
associated completed digest for Need producer construction. It does not rewrite
execution rows with digest keys, and the task-plan row still has no self field.

## 4. Row-kind tags

Each table owns one closed row-kind enum. Existing owner enum order is not used
implicitly; an inherent `semantic_tag()` match writes the explicit tag. Adding
a production enum variant therefore creates a nonexhaustive-match compile
failure until its semantic role is decided. Unknown decoded row-kind tags
reject.

Table 14 has exactly row kind `0 = StaticRuntimeTaskPlanBase`. There is no row
kind for an expected key, alias, compatibility row, or copied View binding.

## 5. Runtime type and expression graph termination

The candidate must already pass structural verification:

- dense table references are in range;
- recursive runtime types use accepted nominal/opaque indirection and do not
  create an unguarded structural cycle;
- expression/flow ownership is a finite table graph with owner-defined roots;
- every child visit is charged to `max_semantic_work`; and
- owner visitors use explicit iterative stacks where recursion depth could
  exceed current Rust stack policy.

The encoder memoizes row digests by `(table_tag, row_ordinal)` and marks a row
`Visiting` before descending. Encountering `Visiting` through a forbidden
structural edge returns `ExecutableSemanticCycle`; accepted nominal references
are leaves by semantic identity. A completed memo entry is reused without
revisiting children, but writing that digest still charges one digest atom.

## 6. Mutation consequences

A mutation to any included semantic role changes the affected owner row digest
and therefore changes `RuntimeExecutableSemanticDigest`. Mutating a task-plan
map key, decoded expected key, debug/source field, or completed plan digest is
impossible in the candidate model; a test-only shadow input demonstrates that
such data is not read by the encoder.
