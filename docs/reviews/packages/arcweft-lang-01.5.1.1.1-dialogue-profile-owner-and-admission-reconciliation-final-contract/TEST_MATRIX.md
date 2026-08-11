# Test matrix

## Status vocabulary

- **DIRECT SOURCE EVIDENCE** — the current baseline source/test was inspected.
- **RECORDED PASS** — the maintained implementation note records the broader
  suite as passing at its implementation cut; this return did not rerun it.
- **CONTRACT REQUIRED** — exact regression case that must remain in the suite;
  existence was not independently enumerated in this return.

## Matrix

| ID | Test owner / API | Input | Exact assertions | Evidence status |
|---|---|---|---|---|
| T01 | workspace dependency test; `cargo metadata` | current workspace graph | `arcweft-manifest-model` has no dependency path to launch/dialogue/view/compiler/runtime-driver; project-loader has no runtime-driver edge; runtime-plan has no compiler edge | CONTRACT REQUIRED |
| T02 | `arcweft-launch`; `SourceBackedManifest::decode` | complete schema-1 manifest consumed by launch/project/compiler/LSP fixtures | decode pass count is exactly one; every consumer holds same document identity and accepted manifest; no second source map | DIRECT SOURCE EVIDENCE for sole decode counter/API |
| T03 | `arcweft-launch` source-map tests | complete dialogue/fallback TOML | each typed path/slot range equals exact substring range and each span's source equals accepted document identity | DIRECT SOURCE EVIDENCE |
| T04 | compiler `dialogue_profile_admission` | dialogue table omitted | accepted View is `std.view.dialogue`; style none; policy `FailLine`; selected View provenance is standard bundled View source | DIRECT SOURCE EVIDENCE |
| T05 | compiler admission | View-only profile with real dialogue-capable View | exact selected View, no style, default policy; retained product `Arc::ptr_eq`; six revision fields match | CONTRACT REQUIRED; success substrate directly observed |
| T06 | compiler admission | Style-only profile with real Style | View defaults to `std.view.dialogue`; exact Style selected; Style provenance and product source revision match | CONTRACT REQUIRED |
| T07 | launch + compiler + runtime-plan | policy-only `discard` or strict fallback | default View/no Style; typed policy preserved through checked profile and runtime-plan codec | CONTRACT REQUIRED |
| T08 | compiler `compiler_admits_profile_against_the_same_view_product_and_revision` | complete View + Style | profile ID, presentation values, exact product Arc, manifest/topology/compiled/program revisions, and View/Style provenance all match | DIRECT SOURCE EVIDENCE |
| T09 | launch strict decoder | valid `inline-failure`; invalid `inline_failure` | valid form decodes; discarded form fails with ordinary `manifest.unknown.field`/unknown-table handling; no alias | DIRECT SOURCE EVIDENCE for strict schema; CONTRACT REQUIRED for exact code fixture |
| T10 | compiler admission | typed `view.Missing` | one error at `DialogueProfileAdmission`; code `profile.dialogue.view.missing`; primary is View value in same manifest | DIRECT SOURCE EVIDENCE |
| T11 | launch nominal decoder | syntactically valid non-View family in `view` | decode fails with `manifest.id.family` at exact scalar; no compiler invocation and no typed View ID | CONTRACT REQUIRED; decoder code/source behavior observed |
| T12 | compiler admission | existing `view.Plain` without dialogue role | code `profile.dialogue.view.not-dialogue`; primary is manifest View value; exactly one secondary is definition source | DIRECT SOURCE EVIDENCE for code; secondary contract from current implementation |
| T13 | compiler admission | `style.Missing` | code `profile.dialogue.style.missing`; primary is Style value; no secondary | DIRECT SOURCE EVIDENCE |
| T14 | launch nominal decoder | syntactically valid non-Style family in `style` | `manifest.id.family` at exact scalar; no typed Style ID crosses boundary | CONTRACT REQUIRED |
| T15 | compiler product/admission unit test | View program source revision or Style program source revision altered away from complete product revision | `profile.dialogue.revision.mismatch`; no checked profile; primary dialogue/profile table | CONTRACT REQUIRED |
| T16 | compiler `profile_admission_requires_the_exact_resource_registry_arc` | separate registry Arc with equal logical contents | rejected as `profile.dialogue.revision.mismatch`; proves object identity is required in addition to digest | DIRECT SOURCE EVIDENCE |
| T17 | compiler product/admission unit test | remove selected View or Style source provenance while definition remains | `profile.dialogue.revision.mismatch`; error variant `MissingSourceProvenance`; selected value primary | CONTRACT REQUIRED |
| T18 | runtime publication transaction | current generation A; candidate B has one mismatched revision field | B rejected; current generation pointer/key remains A; catalog, save header, and all observations still report A's full revision | CONTRACT REQUIRED; atomic behavior recorded by implementation note |
| T19 | CLI/LSP integration | one invalid candidate and one valid candidate | both consume the same `CompiledProject`/structured diagnostic and same source identity; decode counter unchanged; no reparse | CONTRACT REQUIRED; shared-consumer behavior recorded |
| T20 | runtime parity | one complete accepted profile | native/Web/headless/Agent/MCP observe same View ID, Style ID/none, policy, and six-field revision | CONTRACT REQUIRED; parity recorded by implementation note |
| T21 | syntax/parser/HIR tests | source text containing `dialogue defaults` | ordinary parser rejection/recovery; no `DialogueDefaultsItem` typed AST/HIR node; no spelling-specific diagnostic | RECORDED PASS / deletion observed in implementation note |
| T22 | `arcweft-dialogue` revision codec | canonical six-field revision JSON/AWBC/save round trip | exact equality after decode; missing/unknown/noncanonical fields fail | DIRECT SOURCE EVIDENCE for serde JSON; CONTRACT REQUIRED for every transport |
| T23 | `arcweft-dialogue` inline policy codec | unknown fields at top, fallback, or style levels | every malformed value fails; no permissive unit-variant payload | DIRECT SOURCE EVIDENCE |
| T24 | project-default compiler test | compilation without launch-selected profile | owner `ProjectDefault`; profile ID none; standard View; manifest/topology revisions exact | DIRECT SOURCE EVIDENCE |
| T25 | launch resolution test | same accepted manifest resolved twice | equal typed resolved profile, same source map/document; no I/O or source parse in `resolve_profile` | DIRECT SOURCE EVIDENCE for API; CONTRACT REQUIRED for counter assertion |
| T26 | codec strictness | dialogue revision with uppercase/noncanonical digest or legacy field | deserialization fails due typed canonical parsers/deny-unknown-fields | DIRECT SOURCE EVIDENCE |
| T27 | publication replacement | valid candidate B follows rejected candidate | B publishes atomically; no fields from rejected candidate survive; old generation retired only after commit | CONTRACT REQUIRED |
| T28 | behavior parity for policy | fail-line/discard/text/expr-source/call-source/value-plain cases | runtime behavior matches the single dialogue policy enum across native/Web/headless/Agent | CONTRACT REQUIRED |
| T29 | Style same-build invariant | valid ID present in catalog from different source revision | candidate rejected as revision mismatch even though spelling exists | CONTRACT REQUIRED |
| T30 | no parallel registry/catalog | compile and structural API test | only accepted product/catalog path constructs runtime catalog; project-loader cannot name private runtime-driver constructor/capability API | CONTRACT REQUIRED; compile-fail visibility + metadata |

## Required fixture details

All compiler admission fixtures must retain:

```text
Arc<SourceDocument> manifest_document
Arc<SourceDocument> source_document(s)
Arc<SourceBackedManifest> accepted
ProfileId selected profile
SourceSetRevision topology_revision
Arc<ResourceTypeRegistry> accepted/compiler registry
CompiledViewProduct with exact accepted product
```

For source-bound assertions, compare both `SourceDocumentIdentity` and exact
`SourceRange`, not only diagnostic text.

## Required codec assertions

For `DialogueProfileRevision`, assert the canonical wire contains exactly:

```text
manifest_document
topology_sources
compiled_sources
view_program_id
view_program_revision
resource_types
```

Reject missing fields, unknown fields, malformed identity IDs, uppercase
noncanonical source/program revisions, invalid program IDs, and malformed
resource digests.

## Test placement rule

Tests belong with the owning behavior. Do not centralize them in a source-grep
script:

- decoder/source-map/strict TOML: `arcweft-launch`;
- policy/revision codec: `arcweft-dialogue`;
- cross-product admission: `arcweft-compiler`;
- parser typed-node absence: syntax/HIR owners;
- runtime/save/hot replacement: runtime owners;
- dependency graph: workspace structured metadata test;
- CLI/LSP/backend parity: integration/Tier 2 owners.
