# AW-AH-009.3.3 callable catalog shared resolver completion

## Status

The production resolver migration is complete on `main` from base revision
`27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9`. A 2026-07-21 follow-up audit
found that the finalized contract's public immutable target-fact read API is
still crate-private; the completion claim in this note is therefore limited to
resolver/checker integration, not the complete public API contract.

The accepted semantic world, type checker, focused call-fact query, and
signature surface now consume one typed resolver product. All 22 production
`CallableFamily` variants report `SignatureFamilySupport::NativeFacts`; this
denotes resolver/checker fact ownership, not source-surface reachability. There
is no `NotIntegrated` family or compatibility fallback.

## Implemented contract

- Project, environment, lexical, function-value, dialogue, trait-method, and
  data-last candidates are resolved through the shared callable catalog and
  preserve typed identity, source provenance, overload consideration,
  parameter-group coordinates, checked argument slots, result type, effects,
  poison state, and structured diagnostics.
- Registered checking no longer succeeds through the previous parallel
  trait-method or data-last branches. Those paths remain only for standalone
  checking without a `RegisteredSemanticWorld`.
- `TraitCatalog` owns trait projection and callable-schema construction.
- `TypeKind::accepts` owns semantic type compatibility; the duplicate checker
  compatibility module was removed.
- Project callables use accepted HIR source documents and typed declaration
  identities rather than reparsing source text.
- Function-value call facts retain the exact evaluated `TypeKind::Function`,
  including its parameters, result, and effect row.
- Data-last adaptation is a `CallableInstantiation`, not a replacement
  signature validator. It retains the base callable's origin, equivalent
  sources, authority, and schema, and unwraps its base identity when advancing
  into a later curried group.
- Missing focused call facts are reported as
  `SignatureSemanticUnavailable::MissingCallableFacts`; unsupported and
  non-callable surfaces remain typed `NotApplicable` outcomes.

## Direct regression evidence

- Registered DataLast calls retain the selected candidate and exact argument
  mapping.
- A three-group callable can consume its receiver through DataLast adaptation
  and continue into the following curried group.
- Registered trait methods retain precedence over same-spelled DataLast
  candidates.
- Focused function-value facts retain the exact callable type.
- Every production callable family has native signature facts.

The three-group regression exposed a provisional validator leak: the
DataLast-specific validator was incorrectly reused for the next ordinary
function-value call. The final implementation removed that duplicate schema
role and dispatches DataLast behavior from the typed instantiation instead.

## Validation

```bash
cargo fmt --package arcweft-lang-sema
cargo test -p arcweft-lang-sema --lib --no-fail-fast
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-3-3-final-2026-07-20
git diff --check -- crates/arcweft-lang-hir crates/arcweft-lang-sema
```

Results:

- semantic unit tests: 801 passed, 0 failed;
- strict Clippy: passed;
- structural audit: 3,364 files, 1,732 Rust files, 801,912 Rust physical
  lines, 0 errors, 129 warnings.

The structural report is stored under
`docs/implementation/structure-audits/aw-ah-009-3-3-final-2026-07-20/`.
Current warning-level production hotspots include:

- `callable/resolver.rs`: 1,871 physical lines / 1,789 code lines;
- `checker/expr/registered_call.rs`: 2,120 / 2,061;
- `checker/module.rs`: 2,483 / 2,349;
- `traits.rs`: 2,054 / 1,864;
- `callable/schema/families.rs`: 1,261 / 1,206.

They remain below the repository's error threshold and each still represents
one cohesive ownership boundary. Future decomposition should follow
responsibility boundaries rather than split these files mechanically.

`just test-tier2` was not required for this cut. It changes HIR/sema public
contracts across crates, but does not affect a runtime, render, Agent, MCP, or
capture path, so it does not meet the repository's Tier 2 risk predicate.

## Remaining work

The finalized AW-AH-009.3.3 contract requires `CallTargetFacts`,
`CallTargetFact`, checked argument and slot facts, `CallTargetFactError`, and
the corresponding `TypeCheckReport` read APIs to be public. They remain
crate-owned in current production. That visibility/read-model cut must be
completed or explicitly superseded before AW-AH-009.3.3 can be called fully
complete. Downstream work must not reintroduce a parallel resolver while
closing it.
