# Lang-01.5.1.1 dialogue profile presentation-owner intake

## Selected package

The authoritative package is the corrected redelivery
`arcweft-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction-final-contract(1).zip`
with SHA-256
`58bcc3a8b03414e7cca2b08cdd3770517a22b7b09b568a1f34fa2dc34956d506`.
All 18 payload entries in `MANIFEST.txt` were checked for both SHA-256 and
byte size. The package reports `READY_FOR_IMPLEMENTATION`, contains
`OPEN_QUESTIONS.md` as exactly `none\n`, and was designed against Git revision
`c957a61e4a0b9abf094165c41ef4038ce25324c0`.

The earlier archive with SHA-256
`0bd3e0f5ba462c523aef62aca99beeeb196603b7733057933d33beb0308fa0ed`
is superseded. Its useful semantic decision—replace the orphan
`@dialogue.*` owner with real View, Style, and inline-failure types—is retained,
but its physical ownership and wire details are not authoritative.

## Why the corrected redelivery wins

The corrected package matches the accepted repository layering and fixes four
material defects in the earlier result:

1. `DialogueProfileSpec`, the sole schema-1 decoder, and the revision-bound
   generic `ManifestSourceMap` remain in `arcweft-launch`; presentation
   dependencies are not pulled into the neutral `arcweft-manifest-model`.
2. View/Style/resource admission occurs in the compiler transaction after the
   validated product exists. Project-loader does not depend on runtime-driver
   or manufacture a second catalog.
3. Dialogue token paths extend the existing manifest source map instead of
   introducing a separately revisioned dialogue source map.
4. The existing `inline-failure` wire and strict nested
   `InlineFailurePolicy` representation are preserved. The earlier
   `inline_failure`/flattened fallback spelling is discarded with no alias or
   dual reader.

The redelivery also adds the missing nominal-family hardening, typed HIR View
inventory, compiler-owned View lowering, one shared
`Arc<ValidatedViewProduct>`, a complete checked revision tuple, and 64 required
behavioral test rows.

The follow-up
[Lang-01.5.1.1.1](../reviews/requests/2026-07-20-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation.md)
is therefore resolved by this corrected redelivery and must not be dispatched
again.

The later retained
[`Lang-01.5.1.1.1` as-built return](../reviews/packages/arcweft-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation-final-contract.zip)
at SHA-256
`8B7FE4D8DA08B793AB039E612CCE5A27AF3EC34E39B9FA07533C81C1F901350F`
confirms this owner/admission decision against current main and reports
`CURRENT_MAIN_STATE=SATISFIED_BY_CURRENT_IMPLEMENTATION`. It is confirmation,
not authority for a second implementation path; see the
[2026-08-08 correction intake](2026-08-08-lang-01-5-1-correction-returns-intake.md).

## Final semantic contract

The implementation goal is one compatibility-free authority cut:

- `profiles.<id>.dialogue` owns only optional nominal `view`, optional nominal
  `style`, and optional typed `inline-failure` policy;
- pure launch-profile resolution fills only `std.view.dialogue` and
  `InlineFailurePolicy::FailLine` fallbacks;
- `DialoguePresentationProfile` remains the small dialogue-owned resolved
  value;
- the compiler admits the selected profile against the exact shared
  `ValidatedViewProduct`, resource registry digest, manifest identity, source
  revisions, and View-program revision;
- runtime-plan lines contain the final View, profile native Style sheet,
  Character/line rich-text values, inline-failure policy, and accepted revision;
- source `dialogue defaults`, `@dialogue.*`, raw defaults strings, the source
  defaults cascade, and all executable `DialogueDefaultsItem` paths are deleted;
- native/Web/headless/Agent/MCP consumers observe the same mounted View and
  prepared-text result; and
- rejected overlay candidates leave the previous complete generation intact.

No compatibility alias, dual reader, removed-spelling recognizer, source gate,
CSS route, Takumi route, raw TOML reader, generic property bag, or second
dialogue/View/Style/resource registry is permitted.

## Current checkout reconciliation

The current checkout already contains useful final-direction substrate:

- launch-owned `DialogueProfileSpec` with `view`, `style`, and
  `inline_failure` fields;
- dialogue-owned `DialoguePresentationProfile` and
  `ViewId::standard_dialogue()`;
- one `SourceBackedManifest`, decoder, and generic source map;
- the established `ValidatedViewProduct`, native Style resolver, View mount,
  prepared-text, rendering, capture, and observation paths; and
- typed Character dialogue patches and runtime-owned dialogue lifecycle state.

The authority cut is not complete. The first nominal-identity slice now:

- makes authored `view.*` / `style.*` and engine-owned `std.view.*` /
  `std.style.*` construction distinct, while public decoders accept exactly
  those two families;
- rejects both `#` and `@` reference markers before nominal-family checks;
- removes unchecked public `from_public_id` construction; and
- makes `ViewDefinitionRef` own a validated `ViewId` directly;
- decodes View/Style values explicitly as nominal IDs and distinguishes
  `manifest.id.invalid` from `manifest.id.family`; and
- publishes the complete dialogue token family through the existing
  revision-bound `ManifestTokenPath`, with no second source map.

The remaining work is:

- compiler-owned typed View lowering and checked profile admission are absent;
- `RuntimePlanLowerOptions` still contains raw `dialogue_defaults` and a
  separately optional inline policy;
- source `DialogueDefaultsItem` and its syntax/HIR/sema/tooling/runtime
  consumers still exist; and
- maintained content has not yet completed the final profile/View/Style
  migration.

## Cycle-free physical ownership correction

The package places `CheckedDialogueProfile` and `DialogueProfileRevision` in
`arcweft-compiler`, then requires `arcweft-runtime-plan` to carry the revision.
The checkout proves that `arcweft-compiler` already depends on
`arcweft-runtime-plan`; making runtime-plan import a compiler-owned value would
create a dependency cycle.

The semantic six-field revision tuple is authoritative, but its reusable value
type must live on a lower, cycle-free dialogue/presentation boundary reachable
by both compiler and runtime-plan. `CheckedDialogueProfile`, admission, source
labels, and the shared product remain compiler-owned. This is a physical owner
correction only: it must not duplicate the tuple, weaken equality, or create a
conversion shim. The final dependency audit must record the selected lower
owner and prove there is no compiler back-edge.

## Implementation order

1. harden `ViewId`, `ViewStyleSheetId`, and `ViewDefinitionRef` nominal
   ownership and migrate serde/callers;
2. finish final manifest source paths, exact family diagnostics, and canonical
   serialization in the sole decoder;
3. establish the shared revision value and compiler-owned accepted profile
   input;
4. expose the typed HIR View inventory and move the established View lowerer
   from CLI to compiler without source reparsing;
5. admit the profile against the one compiled product and retain the same
   product `Arc` in `CompiledProject`;
6. materialize typed profile/revision data into runtime-plan, View mount,
   save/reload, CLI, LSP, Agent, and MCP;
7. migrate maintained manifests, samples, and current authored-View Tier 2
   expectations;
8. delete the entire source defaults family and raw runtime path in the same
   public authority switch; and
9. satisfy TM-001 through TM-064 plus format, check, strict Clippy, workspace,
   doc, Tier 2, structural, and dependency gates.

## Coordination and evidence

The package depends on the active typed syntax/HIR and rich-text migrations but
does not block their non-overlapping implementation. It must consume the final
typed authority directly rather than add an intermediate reader.

Tier 2 was revalidated on production revision `118a9870` before this migration:
MCP 22/22, Agent observe 1/1, native auxiliary 16/16, and visual 7/7 all
passed. That is baseline evidence only. The complete Tier 2 suite must run again
after the final dialogue-profile authority cut because the package changes the
selected View/Style/revision path.

The nominal-identity/manifest slice has passed `arcweft-id` unit and
compile-fail tests, the complete `arcweft-view`, `arcweft-bundle`, and
`arcweft-launch` test suites, the focused runtime-driver View-runtime suite,
workspace all-target/all-feature check, and workspace strict Clippy. The
structural audit scanned 3,468 files / 1,804 Rust files / 830,297 physical Rust
LOC / 94 manifests with zero errors and 131 existing warnings; exact metrics
are retained under
`docs/implementation/structure-audits/lang-01-5-1-1-nominal-manifest-2026-07-21/`.

The freshly rebuilt `target/debug/arcw.exe` also reached a responsive
`Arcweft Player` window with the exact command
`arcw run --runner native samples/modern-feedback-view/src/main.arcw`. The
former `image.glass_bg` missing-asset failure is not reproducible on the current
binary; the validation process was then terminated and no `arcw.exe` process
was left behind.

## Completion boundary

This package is selected and implementation-ready, not implemented. Completion
requires the old source owner and raw selectors to be absent, one admitted
profile/product/revision generation to reach every consumer, all 64 matrix rows
to pass, and every required broad gate to be recorded without compatibility
behavior.
