# Repository evidence

Audited repository: `Sanzentyo/arcweft`  
Exact main: `004ff3d69f241954eb808985878c348b165a815c`

## Repository-wide policy

- `AGENTS.md` blob `e91f99213dde67953beda6aa078c370a8dc4541d`.
- Typed APIs and original-owner inherent methods are preferred.
- Unreleased compiler/parser contracts are replaced directly.
- No aliases, compatibility modules, wrappers, shims, dual readers, source
  gates, or removed-syntax-only final diagnostics.
- Source/schema/public switches are deletion-driven.

## Current production source

| Path | Blob | Evidence |
|---|---|---|
| `crates/arcweft-lang-hir/src/identity.rs` | `18cb62f57e1d70ec1c79a1b7587af4339d635fed` | private/read-only `HirCallArgumentOrdinal`; central `HirLimit`; qualified ID/liveness validation |
| `crates/arcweft-lang-hir/src/dialogue_application.rs` | `b9c49c78220b934f2356a68132a32e49e987b384` | Dialogue call arguments reuse `HirCallArgumentOrdinal`; ordinary 128 and RichText 32 configuration |
| `crates/arcweft-lang-syntax/src/expr.rs` | `507550a8993047b536d5ad46c41915df35490c02` | `MAX_CALL_ARGUMENTS=128`, `MAX_NESTED_CALLS=32`, recovery/diagnostic limits |
| `crates/arcweft-lang-syntax/src/expr/call_syntax.rs` | `e05187335dafdd6c205770a3c8d41cecde922f55` | exact `CallExpr`, `ArgumentListSyntax`, current forms, type application, path-member evidence, active-slot rule |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | `12ab3bbdca5045d53937c5bd49050c715eb4e103` | complete `CallTargetFacts` and checked argument/slot facts |
| `crates/arcweft-lang-sema/src/callable/limits.rs` | `eac3d3612f0cd4cb8a29da4b440308279153572c` | `CallableLimits`, candidate 256, nesting 32, recovery 256, diagnostics 128, query work 4096 |
| `crates/arcweft-lang-sema/src/checker/call_target_facts.rs` | `ba41789648da04343271dd2a57b9212da3421f28` | focused call facts, current detached syntax consumer, resolver work owner and terminal rollback |
| `docs/implementation/2026-07-21-aw-ah-009-3-semantic-selection-and-resource-accounting.md` | `93e2f1e4bf3b89019191c5919c3263e293502db7` | candidate probing, selected replay, result/fact/signature accounting |
| `docs/implementation/2026-07-24-aw-ah-009-3-static-capacity-associated-callee-blocker.md` | `eb19d1dcd4e59cf91d925d2d1d85a9a3b338228c` | associated route, value-first fallback, zero-resolver bare-generic arity failure, obsolete helper deletion |

## Current syntax facts

`CallArgumentFormSyntax` is exactly:

- `Positional`;
- `Named { name, equals }` — authored as `name = value`;
- `Spread { ellipsis }` — authored as `value...`.

`ArgumentListSyntax::contains_signature_cursor` accepts
`open_paren.end() <= cursor <= content_end`.

`active_argument_slot` counts between separators whose `start <= cursor` and
adds one when the trailing comma `start <= cursor`. Therefore comma starts the
following slot and trailing comma starts one-past.

## Callable facts and limits

`CallTargetFacts` retains expression, source identity, call span, enclosing
callable, target outcome, complete checked arguments and slots, result, effects,
current/next group, function-value type, poison, diagnostics, and active
parameter.

`PRODUCTION_CALLABLE_LIMITS`:

- path segments 32;
- groups 16;
- parameters 128;
- overloads/key 32;
- candidates/call 256;
- nested calls 32;
- recovery nodes 256;
- diagnostics 128;
- source bytes 8,388,608;
- resolver query work 4,096.

This package extends those owners. It does not create a second resolver or
parallel limit record.
