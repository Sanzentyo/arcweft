# Implementation and deletion order

This is a design-only archive. The following is the required compiling implementation order.

1. **Identity inherent behavior.** In `arcweft_lang_hir::identity`, add the exact owner-kind and ordinal matches to the existing `SyntheticRole`; add `SyntheticKey`, retained error variants, accessors, and tests. Keep the already-landed eight `SyntheticOwner` variants unchanged.
2. **Fingerprint transcript.** Add the private owner/raw projection and explicit owner/role tag matches in the same identity module. Add `SyntheticKeyFingerprintInput` and fixed-vector tests. Do not add a numeric slot accessor or hashing dependency.
3. **Transaction integration.** Connect `(SyntheticKey, child HirIdKind)` lookup/reuse, typed owner resolution, staged-owner admission, checked descendant counting, and complete rollback. Exact 1,024 succeeds; the 1,025th fresh descendant fails before publication.
4. **Existing role producers.** Convert all synthetic producers to the table in deterministic order: tails/returns, recovery, contracts, desugaring, patterns/locals, closures/captures, for/if-let/while-let/match, and TypeId elision. Use the original enum's inherent APIs; no local match helper or extension trait.
5. **Postfix candidates.** Connect the two accepted candidate builders last among role producers. Preserve the source-backed postfix owner, root ordinal 0, shared target exclusion, per-kind preorder, unresolved tooling retention, and selected-key non-reuse.
6. **Consumers and fingerprints.** Migrate HIR source/index/debug/cache consumers to the typed key and transcript. Full synthetic-slot fingerprints include the child `HirIdKind` in the higher transcript. Persistent artifacts retain their accepted portable source/project identity and do not persist the session key alone.
7. **One public compiling switch.** Complete the already accepted v6.1.1.4.1.1 public switch and delete any remaining provisional synthetic constructors/readers in the same series. Do not restore `Syntax`, raw IDs, wrappers, dual readers, source reparsing, source gates, or removed-syntax diagnostics.

The current checkout already deleted the raw-owner key and landed typed `SyntheticOwner`; implementation must not recreate the deleted substrate temporarily. Invalid role/owner combinations are programmer/lowering errors, not authoring diagnostics.
