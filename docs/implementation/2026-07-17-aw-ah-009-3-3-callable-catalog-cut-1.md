# AW-AH-009.3.3 callable catalog shared resolver — Cut 1

## Basis and package evidence

This cut implements only the typed substrate and direct invariant gates from
Cut 1 of
`arcweft-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation-final-contract.zip`.
The package was read before implementation, all 11 archive members matched its
manifest, and the archive SHA-256 was
`9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78`.
`OPEN_QUESTIONS.md` contains `none` and the package declares itself ready for
implementation.

The implementation was rebased without conflict onto accepted `main` Git
revision `7b2d8c5ca1af` before final validation. That revision includes the
AW-AH-009.3.2 accepted HIR/request lifecycle. The upstream AW-AH-009.3 call
surface remains the owner of call/argument/range carriers. AW-AH-009.3.2 remains
the owner of accepted-HIR request leasing and cancellation.

## Implemented scope

`arcweft-lang-sema::callable` now owns the Cut 1 in-memory contract:

- validated scalar, index, path, provider, family, and candidate identities;
- typed callable documentation, source evidence, Rust provenance, schemas,
  argument policies, and validators;
- immutable catalog record, set, publication, and read models;
- resolved callable, function-value, non-callable, character-owner, and query
  outcome products;
- checker fact and public semantic-signature result carriers;
- typed diagnostics, build/query limits, exact work accounting, and errors;
- closed production schemas for builtin, FX, Agent, presentation, and dialogue
  call families.

The schemas use typed `TypeKind` values, closed `EffectRow` values, typed
character owners, and existing presentation-handle types. They do not parse or
format type names to decide identity. `SemanticSignatureHelp::try_new`
revalidates the call span, callable source, parameter source, diagnostic span,
and related diagnostic spans against one accepted source-document identity.

Direct tests cover scalar and backing-index boundaries, exact family paths and
near misses, schema continuity/name/rest/default/source invariants, non-empty
catalog and resolved result contracts, active signature and parameter indices,
source-document identity, inclusive work limits, and structural `TypeKind`
method-key equality and hashing.

## Intentional non-goals

This cut does not publish callables from HIR or adapters, add the catalog to the
accepted registered environment, or migrate any current checker target
resolution. It also does not connect AW-AH-009.3.1 call ranges or AW-AH-009.3.2
request lifecycle state. Those are later package cuts and must land in order.
Consequently there is still exactly one successful production resolver: the
existing checker route. The new module supplies validated read-model substrate
only and does not form a second successful resolver.

The package's two-argument `CurriedCallableId::try_new` cannot inspect a base
project/environment schema, while separate prose requires it to reject a
missing schema group. Cut 1 keeps the exact API and context-free checks; the
ownership correction is isolated in
`docs/reviews/requests/2026-07-16-aw-ah-009.3.3.1-curried-callable-group-validation-contract-correction.md`.
Schema existence is enforced by the schema-owning
`ResolvedCallable::try_new` boundary; the shared resolver must publish curried
products through that constructor. The follow-up corrects the contradictory ID
constructor prose and test ownership, not an omitted runtime invariant.

No Serde contract, compatibility alias, deprecated API, migration shim, source
gate, unsafe code, Cargo dependency, feature, or crate-boundary change was
introduced.

## Post-AW-AH-009.3.2 connection assessment

The accepted document/HIR/world lease, freshness stamp, cancellation control,
worker admission, and publication revalidation now exist and are compatible
with this Cut 1 substrate. The rebase produced no Rust conflict or duplicate
callable type.

It is not yet safe to connect semantic callable resolution to LSP. The current
accepted lifecycle deliberately still calls the legacy signature builder, and
its implementation note identifies AW-AH-009.3.1's exact authored-call/range
carrier as the remaining Cut 6 prerequisite. That carrier is not present on
the accepted revision. The accepted registered environment also does not yet
own an atomically published `RegisteredCallableCatalog`; that is AW-AH-009.3.3
Cut 2. Connecting now would either reconstruct syntax/ranges or create a second
successful resolver, both prohibited.

The next safe package work is therefore catalog publication (HIR signatures,
typed adapter publication, and atomic registered-environment publication),
followed by the single checker resolver migration. LSP replacement waits for
both that migration and AW-AH-009.3.1.

## Verification

After the final rebase:

```text
cargo fmt --all -- --check
  PASS

CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-sema --all-targets
  PASS

CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
  PASS

CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-targets callable
  PASS — 28 focused tests, 0 failed

CARGO_INCREMENTAL=0 cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS — 0 errors, 130 warnings
```

The first all-target test attempt exceeded a 60-second command timeout while
linking and produced no failure. The same exact command was immediately rerun
with a larger execution window and passed.

The generated structural reports are in
`docs/implementation/structure-audits/aw-ah-009-3-3-callable-catalog-2026-07-17/`.
The changed crate has unchanged Cargo fan-in/fan-out because no manifest edge
changed. The largest changed production file is
`crates/arcweft-lang-sema/src/callable/identity.rs`: 37,112 bytes, 1,294
physical lines, no embedded test module. It holds the single closed scalar,
path, provider, family, and candidate identity hierarchy. Keeping that exact
hierarchy together is a documented warning-level exception; it contains no
resolver execution, I/O, publication transaction, or tooling logic. The next
largest changed production files are below the 1,200-line warning threshold;
tests are isolated in `callable/tests.rs`.

## Design deviations

The handoff's listed `schema.rs` responsibility is internally decomposed with
`schema/families.rs` so closed family-schema tables do not obscure the common
schema types and constructors. This is an internal responsibility split only;
the intentional public surface remains re-exported from `callable.rs` as the
contract requires.
