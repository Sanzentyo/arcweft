# Implementation and deletion order

This is a design-only archive. Implementation must proceed in these compiling cuts.

1. **Complete the original identity enums.** In `arcweft_lang_hir::identity`, add
   the exact inherent `SyntheticRole` owner/ordinal matches, `SyntheticKey`, errors,
   accessors, and transcript methods. Tail roles match `Expr | Scope`; do not add a
   local helper/extension trait or alter the eight-owner enum.
2. **Retain the fingerprint verbatim.** Implement the explicit owner/role tag
   matches and private numeric projection in the same identity module. Add the two
   fixed vectors and collision tests. Add no decoder, digest dependency, raw
   accessor, or Serde.
3. **Connect transaction liveness and reuse.** Resolve the exact typed owner,
   accept only an already-reserved same-transaction owner, key the ledger by
   `(SyntheticKey, child HirIdKind)`, preflight 1,024 descendants, and guarantee
   complete rollback. Tests use exact `born` and `retired_at` payloads.
4. **Migrate tail producers atomically.** Reserve each source-backed root ExprId or
   body/arm ScopeId before the child; migrate ordinary block/computation/named/
   closure/if/if-let, predicate/proof bodies, and match arms to the producer table.
   Add direct tests before exposing a public switch.
5. **Migrate variable generators with their tests.** Move RecoveryOperand,
   desugared temporary, destructured binding, and closure capture producers to the
   exact checked ordering algorithms. Each producer lands with its `T-GEN-*`
   lowerer/perturbation/boundary tests; identity tests alone are not a cut gate.
6. **Connect postfix candidates.** Preserve AW-AH-009.4.2 root zero, shared target,
   interpretation roles, per-kind preorder, bounded transaction, and selected-key
   non-reuse. Land both direct interpretation tests and one-over rollback.
7. **Migrate consumers and source/fingerprint readers.** The allocated child ExprId
   owns the source insertion. Scope-owned keys do not add a source query. Higher
   fingerprints consume the retained 51 bytes and include child kind in their
   existing full-slot transcript.
8. **One compiling public authority switch.** Complete the accepted expression/
   pattern/type/source publication and migrate sema, verifier, runtime-plan,
   compiler, LSP, formatter, Agent/debug, cache, and project publication. Delete all
   provisional raw/syntax owners, old readers, detached clones, and obsolete
   variants in the same series.

At no stage may implementation restore `SyntheticOwner::Syntax`, a raw-owner key,
an alias, wrapper, compatibility shim, dual reader, source reparse, source gate,
CSS/Takumi path, or removed-syntax-specific final diagnostic. Old Speaker/ContentCall/
stringly Dialogue readers remain frozen until their already accepted replacement can
delete them; this correction does not repair them.
