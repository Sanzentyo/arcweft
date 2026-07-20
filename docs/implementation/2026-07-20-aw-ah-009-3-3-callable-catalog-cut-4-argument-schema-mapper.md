# AW-AH-009.3.3 callable catalog Cut 4 — argument schema mapper

## Scope

This cut continues the accepted callable-catalog/shared-resolver migration
after the first three AW-AH-009.3.3 cuts. It closes the generic unchecked
callable schema and registered argument-mapping portion only. It does not claim
completion of semantic signature help, focused call facts, request
cancellation, LSP projection, or cache migration.

## Implemented contract

An environment `FunctionSignature` whose arguments are intentionally untyped
now publishes one canonical schema:

```text
initial group:
  args: optional rest-positional unchecked

unknown named arguments:
  OpenUnchecked

spread arguments:
  Unchecked

validator:
  Untyped
```

This replaces the former empty ordinary parameter group. Checked signatures
retain their exact ordinary groups and publish `TypedRest` only when an
authored rest parameter exists; otherwise they reject spread arguments.

The registered checker now consumes the schema policy for both
`CallableValidator::Ordinary` and `CallableValidator::Untyped`:

- exact named parameters win before `RestNamed`;
- `Reject` diagnoses unknown named arguments;
- `OpenChecked` and `OpenUnchecked` accept their names while still checking
  the authored value expression once;
- `SpreadArgumentPolicy::Reject` rejects every spread;
- `FixedLiteralOnly` expands only fixed literal spread slots;
- `TypedRest` expands fixed literals and checks a dynamic sequence against the
  rest item type; and
- `Unchecked` does not expand a fixed literal or require a sequence type, but
  still checks the authored expression once.

Overload shape viability uses the same positional, named, rest, and spread
mapping rules as committed argument checking. A rejected spread shape
suppresses the derivative missing-required-argument diagnostic.

The former name-based `event.emit` first-argument exception is deleted.
`event.emit`, `fmt`, custom return-only functions, and every other generic
untyped callable now use the same published schema and check every authored
argument value.

## Direct evidence

Resolver tests cover:

- the canonical schema for both a standard untyped callable and `event.emit`;
- unknown symbols in every authored `event.emit` argument;
- a non-event untyped callable with open named, fixed literal spread, dynamic
  spread, and scalar spread arguments;
- `OpenChecked` accepting an unknown name while checking its value;
- `FixedLiteralOnly` accepting a fixed literal and rejecting a dynamic spread;
- `Reject` rejecting a fixed literal spread; and
- parity between registered and standalone untyped checking.

No source-name dispatch, compatibility schema, deprecated variant, extension
trait, source gate, Cargo edge, or serialized-format change was added.

## Remaining ordered work

The AW-AH-009.3 goal remains open. Later cuts still own:

1. exact source-span-keyed semantic call facts and focused recording;
2. accepted-HIR request acquisition and caller-owned cancellation;
3. typed resolver products for all remaining selected/method/callable families;
4. semantic signature-help result projection;
5. LSP request/cache integration and deletion of the word-based legacy path;
6. full caller/catalog fallback deletion; and
7. the applicable workspace and Tier 2 validation at the public tooling cut.

## Validation

```text
cargo test -p arcweft-lang-sema callable::resolver_tests
  PASS — 16 passed, 0 failed

cargo check -p arcweft-lang-sema --all-targets
  PASS

cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
  PASS

cargo fmt --package arcweft-lang-sema -- --check
  PASS

scoped git diff --check
  PASS

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS — 3,336 files, 1,715 Rust files, 790,914 physical Rust LOC,
  0 errors and 128 checkout-wide warnings
```

The structural audit measured the current files at 1,071 LOC for
`callable/builder.rs`, 810 LOC for `checker/expr/registered_call.rs`, 666 LOC
for `checker/expr/signature_call.rs`, 194 LOC for
`checker/expr/builtin.rs`, 1,031 LOC for the isolated
`callable/resolver_tests.rs` unit-test module, and 2,391 LOC for the existing
`checker/expr.rs` production hotspot. The argument mapper remains below the
1,200-line production warning threshold. This cut touches only two shortened
call sites in the existing `expr.rs` hotspot.

## Design deviations

None within this subcut. The policy mapper follows the returned callable
contract. Work intentionally assigned to later AW-AH-009.3 cuts remains
explicitly incomplete rather than being approximated here.
