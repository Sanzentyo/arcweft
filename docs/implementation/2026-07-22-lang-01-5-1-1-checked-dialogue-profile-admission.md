# Lang-01.5.1.1 checked dialogue-profile authority

Date: 2026-07-22

## Outcome

The compiler-owned `CheckedDialogueProfile` is now the required presentation
owner from project admission through runtime-plan lowering. Source
`dialogue defaults`, `@dialogue.*`, `DialogueDefaultsItem`, and every typed
syntax/HIR/sema/tooling/runtime consumer of that provisional declaration have
been deleted. The current grammar rejects the removed text through ordinary
parsing and recovery; there is no spelling-specific diagnostic or compatibility
reader.

`CheckedDialogueProfile` retains:

- the accepted `ProfileId` and typed `DialoguePresentationProfile`;
- the exact six-field `DialogueProfileRevision`;
- an `Arc` to the same accepted View/Style product retained by
  `CompiledProject`; and
- source provenance for the selected View and optional Style sheet.

Admission re-resolves a launch-selected profile from the same
`SourceBackedManifest`, requires its exact resource-registry `Arc`, and checks
the registry digest also recorded by the accepted View product. Project-default
admission uses the compiler transaction's registry directly. Both owners verify
View/Style product source revisions, resolve the nominal View and optional
sheet, require the canonical dialogue parameter role, and reject missing
provenance. Missing View, non-dialogue View, missing Style, and incoherent
revision failures remain source-bound structured diagnostics.

An omitted manifest dialogue section is resolved before admission to the
standard typed profile: `std.view.dialogue`, no profile sheet, and `FailLine`.
There is no post-admission fallback.

## Runtime authority

`CompiledProject.dialogue_profile` and `CompiledProject.runtime_plan` are both
required accepted products. Low-level runtime-plan lowering accepts an
`AdmittedRuntimePlanLowerOptions` value containing a concrete
`DialoguePresentationProfile` and `DialogueProfileRevision`; callers cannot
omit profile identity. The lower-layer builder remains public as required by
the returned cross-crate API contract, so this is a production-path authority
invariant rather than a claim that arbitrary downstream Rust code cannot
construct a test value. CLI, LSP, bundle, and runtime production consumers do
not call that builder; they retain the compiler-issued `CompiledProject`.

Every lowered `LineDisplaySpec` contains the admitted profile View, optional
profile Style, inline-failure policy, and exact revision. Profile Style is
applied at the mounted dialogue View root before authored View-definition style,
so the cascade has one explicit owner and stable order.

`LineDisplayCatalog` also owns one required revision. Construction and
deserialization reject empty/default identity, unknown fields, and entries with
mixed revisions. AWBC/AWFB product codecs carry the same required catalog.

`ProgramGeneration` and save snapshots retain the exact dialogue revision.
Hot replacement classifies a profile-revision change as a code-compatible
boundary change, and restore rejects generation-level or per-presentation
revision mismatches before mutating the live session.

## Consumer ownership

CLI run, serve, script-test, plan, verify, and bundle paths use the accepted
`CompiledProject` runtime plan. LSP profile state retains the same accepted
project and derives cascade, hover, definition, references, and actions from its
accepted HIR, source identity, and runtime catalog. Neither consumer reparses
source nor re-lowers a detached runtime plan with a fabricated revision.

The single-source `compile_source` convenience path now constructs a real
project source set and delegates to project compilation, so it follows the same
admission and revision rules rather than maintaining a second authority path.

## Fixture migration and remaining boundary

Project-backed Web, reactive-view-style, zundamon, and rich-text grammar
fixtures use manifest-owned profile selection and Character-owned dialogue
style. The eight formerly standalone native/Web fixtures were also moved to
schema-1 projects with `src/main.arcw`, a selected profile, and an explicit
`[profiles.<id>.dialogue]` owner. Their custom View selection and per-speaker
rich-text settings were preserved, and all eight pass compilation through the
accepted manifest path.

The rich-text effects sample previously referred to `@shader.source_glow`, an
external symbol that existed only because one test manually injected it into a
`TypeCheckEnv`. No accepted project resource or schema-1 manifest field owned
that symbol. The two non-admissible sample-local wrapper functions and their
text runs were removed instead of adding an obsolete adapter-manifest field or
an implicit compatibility registration. Typed `Fx.shader` resource validation
remains covered by the integration test that explicitly constructs the shader
registry; built-in shader-tag rendering remains covered by the project sample.
This cut does not claim project-manifest end-to-end coverage for a custom shader
resource. Restoring that coverage depends on the still-pending
[Lang-01.4.2 resource extension-manifest wire correction](../reviews/requests/2026-07-20-lang-01.4.2-resource-extension-manifest-wire-contract-correction.md),
because the current returned package is not repository-specific enough to
define an accepted owner.

Ordinary-function CharacterDialogue application is a separate AW-AH-009.4.2
surface/HIR owner. Until that slice lands, Agent-controller dialogue lowering
cannot be used as end-to-end evidence for this profile cut. When it lands, its
line-task and display catalogs must be merged into the accepted runtime plan
rather than discarded by controller-flow discovery.

## Verification

The final review cut was validated from Jujutsu change `sskpnzsx` with:

```text
cargo fmt --all -- --check                                      PASS
git diff --check                                                PASS
cargo check --workspace --all-targets --all-features            PASS
cargo clippy --workspace --all-targets --all-features           PASS
just test-workspace                                             PASS
just test-tier2                                                 PASS
```

Tier 2 includes all 22 selected MCP stdio cases, the Agent/native auxiliary
capture cases, and four exact visual golden cases. The eight migrated fixture
projects also pass `arcw check --manifest-path ... --profile main` through their
accepted manifests.

The canonical structural audit scanned 3,512 files, 1,821 Rust files, 848,464
physical Rust LOC, and 94 package manifests. It reported 0 errors and 139
warnings. Exact file metrics, dependency edges, public-type duplicates, and
warning details are recorded under
`docs/implementation/structure-audits/checked-dialogue-profile-2026-07-22/`.
Workspace Clippy completed successfully with the existing project-loader
large-error/function-size warnings and one unrelated bundle-test warning.

No source gate is used as acceptance evidence.
