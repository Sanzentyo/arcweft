# Nominal graph argument reconciliation — 2026-09-11

Inspected `main` and `origin/main` at
`c2bc9a6aaafcce473033286849974f9923fe7e02`, with an empty index and the preserved
76-path callable Rust migration. No nominal Rust implementation is changed by
this note. The [C1-C6 gap review](2026-09-09-accepted-rust-nominal-gap-review.md)
remains the implementation status; this note closes a concrete schema omission
found while preparing C1, not the nominal implementation itself.

## Evidence and selected correction

The accepted schema sketch gives each `RuntimeNominalSchemaDefinition` an
identity and body, and each `NominalRef` only an identity. However, the existing
[plan type projection](../../crates/arcweft-core/src/plan/type_kind.rs) requires
ordered `arguments` for a nominal instance. Its child traversal and reference
mapping retain those arguments. The current
[record layout](../../crates/arcweft-core/src/value/nominal_record.rs) and
[runtime semantic facts](../../crates/arcweft-runtime-plan/src/semantic_facts.rs)
also retain the instantiated arguments.

A definition's fields cannot recover this information. An argument can be unused
in an empty record, used repeatedly, or itself reference another nominal. The
semantic digest identifies the instance but is not an invertible argument list.
Filling the plan row with an empty vector or re-querying a separate sema catalog
would violate the accepted complete-graph handoff.

The final core definition therefore retains:

```rust
pub struct RuntimeNominalSchemaDefinition {
    identity: RuntimeNominalSchemaIdentity,
    arguments: Box<[RuntimeTypeSchema]>,
    body: RuntimeNominalSchemaBody,
}
```

Fields remain private. Construction supplies all arguments explicitly, and a
read-only getter exposes their order. `NominalRef` continues to carry only the
typed identity pair; every exact instantiated definition owns its arguments
once. No argument side table or source/display reconstruction is introduced.

Apply the correction throughout C1-C6:

1. Final-analysis projection derives argument schemas from the same exact
   instantiated semantic type as the definition's identity and body.
2. Graph validation and reachability visit arguments in their declared order
   before the body. Nominals reachable only through an argument are retained.
   Repeated argument positions are preserved even when they refer to one node.
3. The version-1 definition transcript encodes nominal identity, semantic
   identity, argument count, ordered argument schemas, then the existing body
   tag and body. Counts use the shared shortest-u32-varint encoder. Reachable
   definitions remain sorted by semantic identity; derived layout hashes and
   unrelated catalog stamps remain excluded.
4. Atomic plan admission projects those argument schemas into the existing
   `RuntimePlanTypeProjection::Nominal.arguments`. Record layouts retain their
   current ordered checked arguments. The graph is still discarded after the
   existing type/domain tables are sealed.
5. Native/AWBC validation and program-bound restore use those same type/domain
   rows. There is no separately reconstructed argument inventory.

The accepted schema sketch also illustrates nominal variant fields directly on
`RuntimeCheckedType::Variant`. Current [checked variants](../../crates/arcweft-core/src/pattern.rs)
already have a shared `owner`, ordered `arguments` and cases, supporting builtin
and nominal owners. Preserve that complete model: add layout to
`RuntimeVariantIdentity::Nominal` as required, and retain the existing variant
owner/argument fields. Do not flatten away builtin ownership or generic arguments.

## Validation and acceptance

Required tests include unused nominal arguments, two arguments with reversed
order, repeated argument positions, distinct instances of an empty generic
record, and a nominal reachable only through a generic argument. Check that
argument-only descendants affect the transitive layout, unrelated definitions
do not, and the exact ordered arguments reach the plan/AWBC type table. Retain
builtin variant and Option/Result tests while adding nominal layout validation.

Performed here: complete accepted nominal design/schema/wire/cut/matrix review
and inspection of the current argument and variant consumers above. No new
Rust acceptance test was run. C1-C6, their full validation matrix and the
[convergence goal](2026-09-08-convergence-goal-plan.md) remain unfinished.

This is a maintained reconciliation of the accepted design's complete generic
graph and consumer requirements. The frozen design files remain byte-identical;
their historical validation is not rewritten. No contract version or live
carrier family changes, and no structural Rust ownership success is published
before the accepted C6 prerequisites.
