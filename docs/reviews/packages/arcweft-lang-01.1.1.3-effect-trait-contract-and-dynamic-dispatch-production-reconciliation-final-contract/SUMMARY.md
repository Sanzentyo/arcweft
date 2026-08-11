# Summary

## Result

`READY_FOR_IMPLEMENTATION` with `OPEN_QUESTIONS=0`.

The current repository has a sound static trait-witness substrate and a shared
callable resolver, but trait methods are not joined to the checked effect
facts: `TraitMethodRequirement` and `TraitMethodImpl` lack an authoritative row
and exact effect-clause source, `TraitCallableId` is a name/local-index identity,
resolved methods are assigned a hard-coded closed-empty row, method values are
rejected, and effect diagnostics still expose `AWF-EFX-*` categories. The
contract closes that production boundary without changing direct suspension,
Stream classification, AWBC, or project nominal semantics.

## Closed decisions

1. **One identity.** `CheckedCallableId` wraps a typed declaration key and its
   exact project/source/standard context. Trait requirements, impl methods,
   inherent methods, imported or reexported bindings, method values, call facts,
   effect graph edges, witnesses, project-index records, and tooling queries all
   carry this ID. `TraitCallableId` and local string callable IDs are deleted.
   Compiler lowering performs one opaque, domain-separated projection into the
   existing general `RuntimeCallableId`; the current runtime trait-method
   identity made from `usize`, trait/method/self-type strings, and a monomorph
   label is deleted and is never a second resolver.

2. **One effect owner.** Final `CheckedCallableFacts` in
   `CheckedCallableCatalog` own each body row and effect contract. Trait-catalog
   records store only the checked callable ID. Resolver schemas and project
   records refer to that ID and query the catalog; they do not copy an effect
   set or infer a body again.

3. **Exact omitted-row rule.** A bodyless authored trait requirement with no
   `effects` clauses and no existing typed tail is created by
   `CallableEffectContract::omitted_bodyless_trait` with
   `EffectRow::closed(EffectSet::new())`. Its exact method-name span is the
   synthetic contract anchor.

4. **Existing row model only.** Authored `effects` clauses provide the concrete
   head of the existing `EffectRow`; an existing typed effect variable supplies
   its open tail. No new row grammar is introduced. A body-bearing method with
   no row uses ordinary inference.

5. **Exact conformance.** The final inferred implementation row is substituted
   and checked by the inherent `EffectRow::check_subset` operation against every
   original declaring requirement. Open tails absorb the complete residual row;
   closed rows reject every residual effect and unresolved actual tail.

6. **Typed diagnostics.** E015 uses
   `sema.trait.effect.omitted_row_missing`; E016 uses
   `sema.trait.effect.closed_row_missing`; E022/E023 use
   `sema.effect.closed_row_missing`. One diagnostic contains sorted missing
   effects and one deterministic shortest typed trace per effect. CLI and LSP
   render the same `EffectDiagnostic` object.

7. **Static witness, not dynamic object.** Parent E017 is
   `SUPERSEDED_FOR_LANG_01_1_1`. Replacement `E017S` proves that a static witness
   call or bound method value reads the original requirement row after typed
   substitution. No `dyn` parser branch, erased type, HIR placeholder, vtable,
   runtime opcode, compatibility node, or special rejection diagnostic is added.

8. **Deletion-driven switch.** Source retention, typed IDs, catalog shells,
   single-pass body facts, conformance, diagnostics, resolver, project index,
   compiler, and tooling migrate in one review stack. The old trait callable ID,
   hard-coded empty method row, copied requirement-as-impl projection, string
   effect IDs, text-only trace, generic `AWF-EFX-001` category, project
   method-value rejection, local-index/string runtime trait identity, and the
   `(usize, String)` witness-method inventory are then deleted before the
   authority switch lands.

## Implementation boundary

This contract requires coordinated changes in `arcweft-lang-syntax`,
`arcweft-lang-hir`, `arcweft-lang-sema`,
`crates/arcweft-compiler/src/trait_methods.rs`,
`crates/arcweft-runtime-plan/src/trait_methods.rs`, the trait-method identity
fields in `crates/arcweft-core/src/plan.rs`, project semantic indexing, CLI
diagnostics, and LSP diagnostics. It does not require a Stream/AWBC opcode
change or the Proof syntax/HIR public switch. Any directly owned runtime-plan
schema/fingerprint update for the replaced trait-method identity lands in this
same cut rather than through a compatibility decoder.

## Validation boundary

This is a design package. Repository source was inspected through the private
GitHub connector at the pinned pushed commit; no production checkout was
modified and no Rust test was run. The archive itself is deterministically
built, member-hashed, and structurally verified. The pushed Git object does not
export the Jujutsu change-id header; this limitation is recorded rather than
filled with an invented value and is not a semantic design blocker.
