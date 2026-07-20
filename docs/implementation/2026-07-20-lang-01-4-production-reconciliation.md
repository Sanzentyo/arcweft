# Lang-01.4 typed resource production reconciliation

## Status

Lang-01.4 is partially implemented. The final private `res` shadow grammar is
complete and recorded in
[`2026-07-17-lang-01-4-typed-resource-shadow-grammar-cut-1a.md`](2026-07-17-lang-01-4-typed-resource-shadow-grammar-cut-1a.md).
The public AST/HIR/sema/tooling switch, descriptor registry, bundle/resource
directory, family lowering, source migration, and old-family deletion remain
open.

This audit compares the implementation-ready package
`arcweft-lang-01.4-typed-resource-final-contract-a8403dcb.zip` with current
production at Git revision `73ef0e886a47`.

## Safe implementation boundary

The following package decisions remain implementation-ready:

- `res` is the only configured-resource declaration keyword;
- `asset` remains a separate packaged-payload identity;
- parser structure is registry-independent;
- resource identity keeps `EntityId`, `PublicId`, and `ResourceTypeId`
  distinct;
- a descriptor registry uses stable schema/field/codec identities;
- source bodies become typed, const-constructible values rather than raw field
  bags;
- existing image/audio/native-motion payload owners are reused;
- old family syntax is removed directly with ordinary current-grammar
  rejection and no compatibility path, source gate, CSS route, or Takumi
  route.

Generic identity and registry substrate may proceed without committing the
affected built-in field schemas.

## Blocking contract contradiction

The package defines `ResourceRef<T>` as an exact reference to a typed `res`
declaration, but its Image, VoiceProfile, Voice, and Rig schemas use that type
for retained Character, View, Action, Layer, Signal, presentation-target, and
scroll-region identities. Those owners are not resource declarations, and the
package's closed `ResourceValueType` has no alternative retained-identity
reference category.

Freezing those descriptors would either reject valid existing identities or
silently weaken `ResourceRef<T>` into a generic entity reference. Neither
outcome is compatible with the package's own identity, directory, save, and
exact-type rules.

The required correction is isolated in:

- [Lang-01.4.1 resource-reference and retained-identity schema contract
  correction](../reviews/requests/2026-07-20-lang-01.4.1-resource-reference-and-retained-identity-schema-contract-correction.md).

The TTS-specific Character/provider portion must consume
AW-AH-009.4.1.2 rather than independently selecting a competing speaker
identity.

## Current implementation order

1. Complete the generic semantic identity and immutable registry substrate
   without publishing contradictory built-in field descriptors.
2. Apply Lang-01.4.1 and AW-AH-009.4.1.2 when their corrected contracts return.
3. Complete public AST/HIR/sema/project-index/tooling migration.
4. Add the typed bundle directory and definition-reference boundaries.
5. Migrate Image first, then the ordered audio/motion/rig families.
6. Atomically migrate repository source and delete all old resource
   recognizers, variants, raw readers, and examples.
7. Run the package's direct matrix, workspace check/Clippy/tests, affected
   parity suites, Tier 2 for runtime/Agent effects, and the canonical
   structural audit.

## Non-goals of this audit

No Rust, schema, codec, fixture, parser recognizer, compatibility layer,
removed-spelling diagnostic, source gate, CSS route, or Takumi route is added
by this note.

