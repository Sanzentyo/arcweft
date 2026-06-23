# Request: Persistent Compiler Query Cache Design

## Request

Please design the implementation path for using on-disk persistent query
records to skip compiler stages across `arcw build` invocations.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

The incremental hot-swap bundle work now has:

- `arcweft-project::{artifact,fingerprint,incremental}` as Sans I/O cache key,
  fingerprint, query, snapshot, and invalidation models.
- `arcweft-project-loader::cache` as a filesystem object/record store with
  package locks, immutable object storage, `.awci` records, stats, verify,
  explain, prune, and release fetch support.
- `arcweft-compiler::project::ProjectCompileCache` as an in-process compile
  unit cache for parsed/linted/lowered module units.
- `arcw build` persisting metadata, runtime-plan, snapshot, and Program AWFB
  artifacts through the filesystem cache adapter.
- `arcw build --watch` retaining an in-memory compile-unit cache for the
  running watch process.

The remaining cross-invocation skip is larger than wiring a record read. The
compiler currently caches `CompiledProjectModule`/`HirModule` values in memory,
but the code explicitly says a persistent adapter should store a stable
serialized unit format, not `HirModule` directly. The current semantic,
typecheck, and runtime-plan passes are still linked-HIR scoped, so pretending
there is fine-grained persistent semantic reuse would be unsound.

This means the repository has the record store and conservative snapshot
evidence, but does not yet have an authoritative stable compiler-object schema
or query execution policy for persistent stage skipping.

## Design questions

Please propose concrete answers for:

1. What is the stable serialized format for persistent parse/interface/HIR
   body/module-object artifacts? Should it be an `.awbo` compiler-private
   format, a set of `.awci` object payloads, or another typed codec?
2. Which data may be persisted across compiler versions, and which must be
   namespaced by exact compiler build identity only?
3. How are `CompiledProjectModule`, `HirModule`, syntax statistics, source
   spans, diagnostics, line-task evidence, and future module-object summaries
   represented without serializing unstable internal Rust structs directly?
4. Which stages are safe to skip before module-aware sema exists? Include
   parse, lint, HIR lowering, interface summary, HIR body, typecheck,
   runtime-plan, bytecode-unit, link-plan, bundle-section, and bundle-index.
5. What validation must happen when reading a persistent record: schema,
   compiler identity, source digest, dependency interface/body digests,
   adapter/environment/profile inputs, and object digest/length checks?
6. How should corrupt records or missing objects be reported and recovered
   from without poisoning the build?
7. How should `BuildSnapshot` record cache hits, misses, corrupt records,
   conservative linked-HIR invalidations, and query reuse decisions?
8. How should `arcw build --watch` combine the in-memory cache with the
   on-disk cache?
9. What CLI reporting and `arcw cache explain` output should prove which query
   was reused and why?
10. What tests prove clean and incremental builds produce the same AWFB content
    root and logical section descriptors?

## Constraints

- Keep `arcweft-project` Sans I/O.
- Keep filesystem reads/writes in `arcweft-project-loader` or CLI/build
  adapters.
- Do not serialize unstable compiler internals as a public compatibility
  promise.
- Do not claim module-aware semantic reuse until resolver/typecheck actually
  operate on module-aware inputs.
- Treat corrupt or stale records as misses unless policy explicitly chooses a
  hard error.
- Prefer typed artifact formats and owner methods over stringly stage logic.
- Preserve deterministic runtime and build output behavior.

## Expected output

Please provide:

- the persistent compiler artifact schema;
- affected crates/modules;
- new or changed public/private types;
- the query execution/read-through/write-through policy;
- the interaction between in-memory watch cache and filesystem cache;
- the `BuildSnapshot` evidence model;
- recovery behavior for missing/corrupt records;
- step-by-step implementation order;
- focused tests and smoke commands for each step.

## Current goal boundary

Until this design is answered, the current incremental hot-swap goal should not
implement:

- cross-invocation persistent reuse of parse/HIR/typecheck/runtime-plan
  compiler query artifacts;
- a stable `.awbo` or equivalent compiler-private unit codec;
- serializing `HirModule` or `CompiledProjectModule` directly as a long-term
  cache format;
- claiming fine-grained persistent semantic reuse while the semantic pass is
  still linked-HIR scoped.

The current goal may keep:

- filesystem `.awci` object/record storage;
- persisted build output records for metadata, runtime-plan, snapshot, and
  Program AWFB;
- verified Program AWFB cache reuse for repeated identical project builds;
- in-memory compile-unit cache reuse inside one `arcw build --watch` process.

## Useful current evidence

Start with these files:

- `crates/arcweft-project/src/artifact.rs`
- `crates/arcweft-project/src/fingerprint.rs`
- `crates/arcweft-project/src/incremental.rs`
- `crates/arcweft-project-loader/src/cache.rs`
- `crates/arcweft-project-loader/src/cache/store.rs`
- `crates/arcweft-compiler/src/project.rs`
- `crates/arcweft-compiler/src/incremental.rs`
- `crates/arcweft-cli/src/app/project_commands.rs`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
