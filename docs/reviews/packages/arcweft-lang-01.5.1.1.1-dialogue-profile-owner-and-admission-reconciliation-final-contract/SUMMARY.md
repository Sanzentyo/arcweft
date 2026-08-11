# Executive summary

## Result

All result-changing decisions are closed.

```text
DESIGN_STATUS=READY_FOR_IMPLEMENTATION
CURRENT_MAIN_STATE=IMPLEMENTED
OPEN_RESULT_CHANGING_DECISIONS=0
BASELINE_GIT_COMMIT=0c8cb74dd96116a8b987cc419c9a280b6cabe4a4
```

## The selected boundary

The only accepted path is:

```text
one SourceDocument
  -> one SourceBackedManifest::decode
  -> one launch-owned typed manifest plus generic source map
  -> pure DialoguePresentationProfile resolution
  -> project topology freeze
  -> compiler View/Style product construction
  -> compiler-owned CheckedDialogueProfile::try_admit
  -> one six-field DialogueProfileRevision
  -> one CompiledProject and runtime plan
  -> atomic ProgramGeneration publication
```

There is no parallel decoder, profile catalog, View catalog, source map,
resource registry, revision identity, or compatibility bridge.

## Exact six-field equality authority

`DialogueProfileRevision` is structurally equal only when all of these typed
facts are equal:

1. `SourceDocumentIdentity manifest_document`
2. `SourceSetRevision topology_sources`
3. `SourceSetRevision compiled_sources`
4. `ViewProgramId view_program_id`
5. `AcceptedViewProgramRevision view_program_revision`
6. `ResourceTypeRegistryDigest resource_types`

Admission additionally requires exact `Arc` identity for the launch-selected
resource registry, not merely an equal digest, and retains the exact accepted
`Arc<ValidatedViewProduct>`.

## Exact source and wire authority

- authored field: `inline-failure`
- discarded spelling: `inline_failure`, rejected as `manifest.unknown.field`
- default: `std.view.dialogue`, no style, `fail_line`
- source ranges: typed `ManifestTokenPath` plus `ManifestTokenSlot` through the
  existing generic `ManifestSourceMap`

## Status of the old request

The source request is resolved and must not be re-dispatched. This package
preserves its required archive shape while updating the disposition to reflect
current source reality and the current Git-only repository policy.
