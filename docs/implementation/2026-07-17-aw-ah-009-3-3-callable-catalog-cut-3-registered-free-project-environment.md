# AW-AH-009.3.3 callable catalog Cut 3 — registered free project/environment path

## Basis and completion boundary

This is a coherent sema subcut of
`arcweft-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation-final-contract.zip`
(SHA-256
`9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78`).
It was developed from accepted `main` Git revision `009e33b7` in Jujutsu
change `zwwknptw`.

This note records the **registered free project/environment path cut** only.
It does not claim completion of AW-AH-009.3.3. The preceding Cut 1 and Cut 2
notes remain the evidence for the typed catalog substrate and atomic catalog
publication respectively.

## Implemented scope

`arcweft-lang-sema::callable` now owns the exact crate-private resolver request
shape selected by the contract:

- `CallCallee`, `ResolvedFunctionValueSeed`, `CallSourceContext`,
  `LexicalCallableScope`, `LexicalCallBinding`, and `CallResolverRequest`;
- validation of accepted symbol-world/revision agreement, registered
  environment agreement, current-module source identity, optional source
  spans, source-size limits, and cancellation;
- one `resolve_call_target` entry with checked resolver work accounting;
- free-call lookup through a one-segment lexical snapshot, the exact project
  binding in the current module, and the accepted environment catalog;
- terminal project non-callable shadowing before environment lookup;
- typed project, standard, and adapter `SignatureOrigin` construction from
  accepted catalog records, including catalog reachability checks.

The registered type checker retains the accepted `RegisteredSemanticWorld`,
constructs one request for a free path call after higher-priority
language-owned families, and uses `RegisteredCallableCatalog` candidates for
project, standard, and adapter calls. Ordinary schema argument checking covers
current positional/named/fixed-literal-spread/typed-rest, required/default,
result, and effect behavior for this path. Virtual-path validation remains a
validator attached to the resolved ordinary candidate.

The checker surface accepts a free `CallablePath` only by recursively walking
structured `Expr::Path` and `Expr::Select` nodes and validating every retained
segment as `CallableName`. It does not split or reparse a rendered path. This
connects parser-produced `custom.read(...)` selectors to the exact two-segment
adapter catalog key. A selector whose root already has lexical/value type
evidence remains owned by selected-call checking, so a local `item.len()` is
not captured by a same-spelled environment free candidate `item.len`.
Non-path receiver expressions never enter the free resolver probe.

In registered mode, the former successful `TypeCheckEnv` function-map lookup
is no longer a fallback for this migrated path. The direct adapter-only test is
not present in that map and therefore proves successful catalog consumption.
Unregistered standalone type checking keeps its existing function inventory;
removing that inventory and its one-time core catalog projection belongs to a
later complete legacy-deletion cut.

Direct resolver tests prove:

- typed project, standard, and adapter candidate IDs and origins;
- single-segment and two-segment adapter free candidates plus registered
  checker success for both shapes;
- selected-call precedence for a local `Vec<i32>` receiver when an environment
  dotted free candidate has the same `item.len` spelling;
- terminal project non-callable shadowing;
- rejection of the wrong accepted document and a span owned by another
  document;
- rejection of mixed symbols/world generations as `WorldMismatch`;
- fail-closed cancellation;
- resolver work limit zero producing a typed `Work` rejection at the first
  query step with no partial candidate result and no consumed work;
- registered checker success for project, standard, and adapter-only calls.

## Explicit remaining ordered cuts

The overall AW-AH-009.3.3 goal remains open. This cut intentionally does not
claim or implement:

1. transactional overload viability based on inferred argument types,
   candidate checkpoints, specificity, ambiguity products, and one committed
   diagnostic/effect result;
2. selected/method families, including environment, collection,
   presentation-handle, integer, domain, capacity, trait, and data-last
   precedence;
3. dialogue target resolution and final structural character-owner expected
   types;
4. lexical callable, curried function-value, speaker, and ordinary evaluated
   function-value migration through the shared checker engine;
5. checker target-fact recording, focused-call facts, semantic signature-help
   projection, and LSP connection;
6. migration of every remaining language-owned free-call family and deletion
   of every legacy checker dispatch branch;
7. the typed external dotted project-binding redesign isolated in
   `docs/reviews/requests/2026-07-17-aw-ah-009.3.3.2-typed-external-project-binding-path-publication.md`.

The compiler typed-publication passthrough is deliberately not duplicated in
this sema cut; it is owned by the independent accepted-HIR/compiler integration
work. No source-text identity parsing, compatibility alias, dual registered
resolver, deprecated API, source gate, unsafe code, Cargo edge, or serialized
format was introduced.

## Verification

```text
cargo fmt --all -- --check
  PASS

cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
  PASS

cargo test -p arcweft-lang-sema callable::resolver_tests --no-fail-fast
  PASS — 8 tests, 0 failed

cargo test -p arcweft-lang-sema --lib --no-fail-fast
  PASS — 675 tests, 0 failed

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS — 3,158 files, 1,588 Rust files, 726,628 Rust physical LOC,
  0 errors and 128 warnings

jj diff --git --color never |
  git apply --check --reverse --whitespace=error-all -
  PASS
```

## Structural audit

No Cargo manifest changed. `arcweft-lang-sema` retains fan-out 12 and fan-in
11. All changed Rust files belong to that crate. Measurements below are from
the current checkout, not diff additions; none contains an embedded
`#[cfg(test)]` module. `resolver_tests.rs` is an isolated crate unit-test
module selected by `callable.rs` under `#[cfg(test)]`.

| Path | Bytes | Physical LOC | Classification and responsibility |
|---|---:|---:|---|
| `src/callable/catalog.rs` | 16,205 | 467 | production; immutable catalog reads |
| `src/callable/limits.rs` | 8,088 | 277 | production; exact build/query work accounting |
| `src/callable/resolver.rs` | 35,226 | 1,104 | production; validated request, free probes, typed resolver products |
| `src/callable/resolver_tests.rs` | 15,797 | 444 | unit test; resolver/request/catalog/checker behavior |
| `src/callable.rs` | 4,421 | 85 | production facade; intentional crate-private resolver surface |
| `src/checker/expr/callable.rs` | 16,808 | 440 | production; function-value/path call checking boundary |
| `src/checker/expr/registered_call.rs` | 19,208 | 526 | production; structural free-path extraction and registered candidate validation |
| `src/checker/expr.rs` | 95,235 | 2,492 | production hotspot; expression dispatch, touched only to delegate registered free calls |
| `src/checker/module.rs` | 88,842 | 2,343 | production hotspot; module analysis orchestration, touched only to pass the accepted world |
| `src/checker.rs` | 63,331 | 1,763 | production hotspot; checker state/construction, touched only to retain the accepted world |

The canonical audit reports the last three files as existing warning-level
size hotspots. This cut does not add their resolver implementation inline:
the 526-line `registered_call` responsibility module owns that work, and
the 1,104-line resolver remains below the 1,200-line production warning
threshold. The largest non-generated workspace Rust file is the existing
7,970-line CLI integration test
`crates/arcweft-cli/tests/check/cli_runtime_bench.rs`; the largest production
file is the existing 2,500-line `crates/arcweft-core/src/value.rs`. The audit
found no error-level structural violation.

## Design deviations

The registered checker uses a small internal `RegisteredFreeCallOutcome`
instead of nested `Option<Option<TypeKind>>`, so “not this resolver” is distinct
from “handled with a poisoned/no type result.” This does not change the package
public model.

The exact `LexicalCallBinding` and accepted query products retain their large
typed function-value payloads inline as specified. Narrow clippy allowances
document those exact-contract cases; no boxed compatibility wrapper was added.
