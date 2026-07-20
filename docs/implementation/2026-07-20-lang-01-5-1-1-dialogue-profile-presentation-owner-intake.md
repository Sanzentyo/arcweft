# Lang-01.5.1.1 dialogue profile presentation-owner intake

## Package and status

The source package is
`arcweft-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction-final-contract.zip`
with SHA-256
`0bd3e0f5ba462c523aef62aca99beeeb196603b7733057933d33beb0308fa0ed`.
The package is design-only, reports `READY_FOR_IMPLEMENTATION`, and has no
result-changing open questions.

The implementation goal is one atomic owner migration:

- profile dialogue presentation is the typed tuple of `ViewId`, optional
  `ViewStyleSheetId`, and `InlineFailurePolicy`;
- source `dialogue defaults`, `@dialogue.*`, and every executable
  `DialogueDefaultsItem` path are deleted;
- the sole revision-bound manifest decoder owns the exact accepted TOML shape
  and ranges;
- accepted topology admission validates View, dialogue capability, Style,
  resources, and source/View revisions before publication; and
- runtime, LSP, Agent/MCP, save/reload, and command consumers borrow the same
  accepted topology without reparsing the manifest.

No compatibility alias, dual reader, removed-spelling recognizer, source gate,
CSS path, Takumi path, generic property bag, or second dialogue/View/Style
registry is permitted.

## Current checkout reconciliation

The current `main` already contains part of the final substrate:

- `ViewId::standard_dialogue()` owns `std.view.dialogue`;
- `arcweft-dialogue::DialoguePresentationProfile` owns the resolved typed
  presentation policy;
- `ResolvedLaunchProfile::dialogue()` exposes that typed value;
- the launch decoder accepts the final `[profiles.<id>.dialogue]` table and
  strict inline-failure variants, and rejects the removed manifest
  `dialogue.defaults` field; and
- the manifest selection path now uses one containing project manifest for
  direct source launches.

The atomic correction is not complete:

- `DialogueProfileSpec` and its decode ownership still sit in the launch
  layer rather than the package's final authored-manifest owner;
- `DialogueDefaultsItem`, its parser, AST/HIR/sema paths, tooling and LSP
  features, runtime-plan selection, and source-default cascade still execute;
- `RuntimePlanLowerOptions` still carries a raw `dialogue_defaults` string;
- maintained source fixtures and tests still author `@dialogue.*`; and
- accepted project topology does not yet own the package's
  `CheckedDialogueProfile` revision tuple.

The existing typed launch fields are final-direction substrate, not a reason
to preserve the old source owner.

## Required implementation transaction

The package's migration order remains authoritative:

1. finish the exact authored manifest types and source-map diagnostics in the
   sole decoder owner;
2. retain one resolved `DialoguePresentationProfile`;
3. add checked topology admission and exact source/View/Style revision
   evidence;
4. move compiler, project index, runtime-plan, command, LSP, Agent/MCP,
   save/reload, and overlay consumers to that accepted value;
5. migrate maintained manifests, Arcweft source fixtures, and Tier 2
   expectations;
6. delete the complete source `dialogue defaults` family and raw runtime
   selection path in the same public cut; and
7. run every TM-001 through TM-064 behavior plus workspace, doc, parity,
   Tier 2, and structural gates before claiming completion.

## Coordination boundary

This transaction directly overlaps the active Proof typed-AST/HIR public
switch, AW-AH-007/008 rich-text HIR/runtime migration, and AW-AH-009.3 LSP
request cut. It must therefore not publish a second temporary AST/HIR reader
or retain `DialogueDefaultsItem` behind a compatibility carrier. The safe
order is:

1. complete Proof Stage 2 private identity/attachment without changing the
   public reader;
2. land the AW-AH-007/008 lossless syntax slice;
3. complete the Proof atomic public syntax/HIR switch;
4. port the final dialogue-profile owner directly onto that typed surface; and
5. perform the source-default deletion and downstream migration as one
   compile-clean transaction.

This dependency order does not block the other active implementations. It
prevents this package from creating an intermediate public model that Proof
would immediately delete.

## Validation already relevant

The current accepted launch path has direct decoder and native-launch
evidence. The MCP stdio Tier 2 group was rerun on `main` after the containing
manifest and View-handler ownership corrections and passed 22/22. This is
baseline evidence only; the final dialogue-profile transaction must rerun the
complete `just test-tier2` recipe and the package's native/Web/headless parity
matrix.

## Completion boundary

This package remains implementation-ready and active. It is not complete until
the old source owner and all raw selectors are absent, the checked topology
revision tuple is published atomically, all 64 matrix rows pass, and the
required broad validation is recorded.
