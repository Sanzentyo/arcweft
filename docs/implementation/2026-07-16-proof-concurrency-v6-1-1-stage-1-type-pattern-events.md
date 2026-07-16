# Proof-concurrency v6.1.1 Stage 1 type and pattern events

## Scope

This private Stage 1 slice follows Git `0577365ba120` and advances the
proof-concurrency v6.1.1 shared-cursor grammar. It changes no public parser,
typed AST, HIR, semantic, compiler, or runtime behavior and allocates no
production syntax identity.

## Implemented ownership

- one shared path emitter now owns full `Path` identity and ID-less path
  segments for expression, type, and pattern grammar;
- function, sum, reference, tuple, slice, array, generic-application, type
  argument, primitive, lifetime, inferred, and path types recursively emit
  their authored descendants from the one document cursor;
- tuple, sequence, record, variant, rest, whole-binding, or, mutable-binding,
  literal, entity-reference, wildcard, binding, missing, and error patterns
  recursively emit independent pattern and field nodes;
- record fields distinguish shorthand bindings from explicit field-pattern
  children, while variant tuple/record payloads reuse the same pattern
  authority; and
- `where` predicates now own typed subject and bound descendants rather than a
  flat token span.

No source substring is reparsed. Every real token is emitted exactly once in
source order, and the validated green text remains byte-for-byte equal to the
input document.

Direct coverage combines a function type, reference and lifetime, nested
generics, tuple and anonymous sum types, fixed-size array, nested tuple/list/
record patterns, mutable record field, two rest patterns, a whole binding,
three variants, an or-pattern, and nested where bounds on one declaration.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax --lib
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests::nested_type_and_pattern_families_have_independent_events --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

The first combined workspace invocation used an orchestration timeout that was
too short and was terminated after five seconds before producing a result. It
was rerun with a three-minute command allowance and completed successfully in
171.9 seconds.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-type-pattern-events-2026-07-16/`.
It scanned 2,934 files, 1,455 Rust files, 679,107 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope parser file.

The first audit invocation included `--fail-on-violations` and exited 1 because
that mode also rejects the 129 existing warnings; it still wrote a zero-error
report. The canonical write was rerun without that flag and completed
successfully with the same zero-error result.

Exact current metrics are:

- `parser/expression.rs`: 15,403 bytes / 488 physical LOC / 459 code LOC;
- `parser/path.rs`: 1,693 bytes / 54 physical LOC / 48 code LOC;
- `parser/pattern.rs`: 12,018 bytes / 363 physical LOC / 340 code LOC;
- `parser/predicate_proof.rs`: 14,738 bytes / 435 physical LOC / 411 code LOC;
- `parser/type_ref.rs`: 9,659 bytes / 296 physical LOC / 279 code LOC.

All are production files without embedded tests or generated content.
`path.rs` is deliberately small because it centralizes only the one full-path
ownership rule shared by three responsibility modules.

## Remaining boundary

This is not the complete private grammar gate. Detailed expression control
families and recovery, all shared statement families, remaining item families,
depth-zero multiline ownership, syntax limits, and the later attachment/public
syntax/HIR/project/runtime stages remain open.
