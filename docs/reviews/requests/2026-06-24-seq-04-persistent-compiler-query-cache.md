# Request 04: Persistent Compiler Query Cache

## Sequence Position

This is the fourth design request in the integrated execution sequence.

Submit this after:

- `2026-06-24-seq-01-executable-runtime-core.md` has stabilized AWBC/runtime IR
  artifact identities;
- `2026-06-24-seq-02-product-artifact-patch-signing.md` has stabilized section
  schema and content-root/fingerprint identities enough to reference build
  outputs.

## Request

Please design the persistent compiler query cache for cross-invocation compiler
stage reuse. The design must respect the already implemented `.awbo` object
contract, avoid serializing unstable Rust internals directly, and avoid claiming
fine-grained semantic reuse before module-aware sema exists.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Request Files To Incorporate

Use this existing request as source material:

- `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`

Also incorporate the implemented contracts recorded in:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-project/src/persistent_object.rs`
- `crates/arcweft-project/src/artifact.rs`
- `crates/arcweft-project/src/fingerprint.rs`
- `crates/arcweft-project/src/incremental.rs`
- `crates/arcweft-project-loader/src/cache.rs`
- `crates/arcweft-project-loader/src/cache/store.rs`
- `crates/arcweft-compiler/src/project.rs`
- `crates/arcweft-compiler/src/incremental.rs`

## Why This Comes After Requests 01 and 02

Persistent cache keys and payloads depend on stable artifact identities:

- bytecode-unit and link-plan reuse depend on AWBC/runtime IR design;
- bundle-section reuse depends on product section schemas and content roots;
- runtime-plan reuse depends on linked-HIR and semantic boundaries;
- fine-grained typecheck reuse is unsound until module-aware sema exists.

Designing this first would bake current transitional structures into persistent
objects.

## Current Implementation Evidence

The repository currently has:

- Sans I/O project artifact keys, fingerprints, query kinds, snapshots, and
  invalidation models;
- filesystem-backed `.awci` object/record storage with lock, stats, verify,
  explain, prune, and release fetch support;
- in-memory `ProjectCompileCache` for watch builds;
- build output records for metadata, runtime-plan, snapshot, and Program AWFB;
- compiler-private `.awbo` envelope/key/payload contracts for parsed syntax,
  interface summaries, HIR bodies, line-task evidence, runtime-plan units,
  bytecode units, and link plans.

## Required Design Decisions

Please provide concrete answers for:

1. What exact `.awbo` payload codecs are required for parse, lint/interface,
   HIR body, line-task evidence, runtime-plan unit, bytecode unit, and link
   plan artifacts?
2. Which data is stable across compiler versions, and which must be namespaced
   by exact compiler identity?
3. How are syntax statistics, source spans, diagnostics, interface summaries,
   HIR body facts, line-task evidence, and module-object summaries represented
   without serializing `HirModule` or `CompiledProjectModule` directly?
4. Which stages are safe to skip before module-aware sema exists?
5. What validation must happen when reading a record and object?
6. How are corrupt records, missing objects, compiler identity mismatches,
   digest mismatches, and dependency mismatches reported as soft misses?
7. How does `BuildSnapshot` record cache hits, misses, stale records, corrupt
   objects, conservative linked-HIR invalidations, and rebuild reasons?
8. How does `arcw build --watch` combine in-memory cache and disk cache?
9. What should `arcw cache explain` report for a query key or logical item?
10. How do cache reads/writes remain deterministic and adapter-owned?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Add `.awbo` deterministic encode/decode for parse and HIR-body exact
   compiler-private objects.
2. Add read-through validation that treats stale/corrupt/missing as soft miss.
3. Record query reuse evidence in `BuildSnapshot`.
4. Add write-through for safe parse/HIR objects.
5. Add `arcw cache explain` query-level evidence output.
6. Add interface summary reuse after its schema is explicit.
7. Add runtime-plan/bytecode/link-plan reuse only after executable runtime
   identities from Request 01 are stable.
8. Explicitly report typecheck/runtime-plan linked-HIR conservative misses
   until module-aware sema exists.

## Tests To Specify

The design should include focused tests for:

- deterministic `.awbo` encode/decode;
- compiler identity mismatch soft miss;
- missing object soft miss;
- corrupt record soft miss;
- object digest/length mismatch soft miss;
- repeated clean build produces same AWFB content root;
- watch in-memory cache precedence over disk cache;
- `arcw cache explain` shows key inputs and hit/miss reasons.

## Constraints

- Keep `arcweft-project` Sans I/O.
- Keep filesystem reads/writes in `arcweft-project-loader` or CLI/build
  adapters.
- Do not serialize unstable compiler internals as a public compatibility
  promise.
- Do not claim module-aware semantic reuse until sema supports it.
- Prefer typed artifact formats and owner methods over stringly stage logic.

## Expected Output

Please produce one design document with:

- persistent object schema;
- query read-through/write-through policy;
- cache validation and soft-miss behavior;
- BuildSnapshot evidence model;
- `arcw cache explain` output;
- implementation cuts;
- test plan;
- explicit non-goals.

