# Direct deletion and residue matrix

This matrix is normative. Every listed item is deleted in the indicated
compiling cut. A replacement column names a real final owner; `none` means the
legacy concept has no successor. No item receives a deprecated alias, re-export,
dual decoder, temporary public wrapper, or spelling-specific migration branch.

## 1. Request-mandated deletions

| Legacy concept | Current owner / role on inspected main | Final replacement | Deletion cut | Direct proof |
|---|---|---|---|---|
| `Character.say(...)` | language docs, callable/member handling, samples | direct `Ref<Character>(patch)` factory plus bracket/colon content application | Cut 3, all samples/docs Cut 7 | ordinary method resolution rejects `.say`; direct forms type-check |
| `SpeakerPreset.say(...)` | old dialogue model/docs | none | Cut 1 | compile-fail public API test |
| `SpeakerPreset.call(...)` | old dialogue model/sema | none | Cut 1/3 | compile-fail type/API test |
| `Speaker` | source/prelude role and sema classification | `Ref<Character> | CharacterDialogue` only at the content-application resolver boundary; no alias type | Cut 3 | type lookup/compile-fail test |
| `SpeakerRef` | `arcweft-dialogue` public wrapper around `PublicId` | `CharacterId` / `Ref<Character>` | Cut 1 | public API compile-fail; CharacterId construction test |
| `SpeakerPreset` | `arcweft-dialogue` configured line value | nominal `CharacterDialogue` | Cut 1/3/4 | public API compile-fail; runtime value round-trip |
| `DialogueSpeakerPreset` | runtime-plan static let scan/preset payload | none; typed `CharacterDialogue` runtime values flow through ordinary expressions | Cut 4 | branch/return/capture/indirect-call runtime tests |
| `SayOptions` | `arcweft-dialogue` untyped/optional option record | `CharacterDialogueConfig` plus checked `CharacterDialoguePatch` | Cut 1 | compile-fail and merge-table tests |
| `DialogueLineBuilder::say()` | Rust builder entry | `CharacterDialogue::apply_content` through the owned domain execution context; source uses brackets/colon | Cut 1/4 | compile-fail and source/runtime parity tests |
| `TypeKind::Speaker` | sema role variant | `TypeKind::Ref(Character)` for character references | Cut 3 | exhaustive type-order/mismatch/label tests |
| `TypeKind::SpeakerPreset` | sema configured-value variant | `TypeKind::CharacterDialogue(CharacterDialogueType)` | Cut 3 | exhaustive type tests and compile-fail |
| `DialogueCalleeIdentity::Speaker` | shared dialogue resolver identity | `DialogueCalleeIdentity::Character` | Cut 3 | resolver candidate identity tests |
| `DialogueCalleeIdentity::SpeakerPreset` | shared dialogue resolver identity | `DialogueCalleeIdentity::CharacterDialogue` | Cut 3 | resolver candidate identity tests |
| `DialogueCallableId::SpeakerLine` | common dialogue call surface | `CharacterFactory`, `CharacterReconfigure`, `ContentApplication` | Cut 3 | schema inventory/exhaustive resolver tests |
| `speaker_preset_chain` | runtime-plan lexical-name inheritance | none | Cut 4 | dynamic-value tests; no static preset inventory input |
| all `.say` suffix stripping or reconstruction | HIR line identity, runtime-plan, tooling | typed source target facts and source-site line identity | Cut 2/4/6 | generated-ID tests and canonicalizer output tests |

## 2. Additional production residue that must disappear

| Legacy residue | Final handling | Cut | Evidence |
|---|---|---|---|
| `SpeakerLine` | replaced by `DialogueContentApplicationExpr` with colon surface | Cut 2 | parser AST/range tests |
| `SpeakerLineSurface` | replaced by `DialogueContentApplicationSurface::Colon` | Cut 2 | exact range/recovery tests |
| `ContentCall { callee: String, ... }` | replaced by expression-target content application | Cut 2 | AST/HIR structural tests |
| `HirDialogue.callee: String` | deleted; HIR owns `AuthoredExpr` target | Cut 2 | HIR snapshot/API tests |
| `HirDialogue.speaker_surface` | deleted; one application surface enum | Cut 2 | HIR source-map tests |
| `DialogueSpeakerSlug` | deleted | Cut 2 | source-site ID tests |
| speaker-derived generated line-ID prefixes | replaced by typed source-owner/scope/ordinal prefixes | Cut 2 | exact/collision/rename tests |
| `.say`-derived text-key logic | text key derives only from final `RuntimeLineId` | Cut 2 | text-key family tests |
| `SpeakerLineType` | deleted | Cut 3 | sema exhaustive match/compile-fail |
| `SpeakerLineOutcome` | replaced by checked Character/CharacterDialogue target facts | Cut 3 | canonicalization inventory tests |
| `CheckedSpeakerLine` | replaced by `CheckedCharacterDialogueApplication` | Cut 3/6 | exact accepted-fact tests |
| `speaker_line_classification` | replaced by inherent typed CharacterDialogue target/call resolution | Cut 3 | direct resolver tests |
| checker branches returning `SpeakerPreset(Character)` | replaced by shared schema resolution and `CharacterDialogueType` | Cut 3 | path/local/call tests |
| `DialogueCallableId` schema branch shared by speaker/preset | split into the three fixed call surfaces | Cut 3 | parameter-group/signature-help tests |
| `speaker_preset_from_let` | deleted | Cut 4 | let/branch/function/closure runtime tests |
| preset-name lookup/reverse scanning | deleted | Cut 4 | shadowing and same-spelling-module tests |
| `DialogueDisplayDefaults.characters: BTreeMap<String, ...>` as runtime identity | compile to CharacterId-keyed defaults table | Cut 4 | alias/collision/defaults tests |
| `character_labels: BTreeMap<String, String>` as identity route | compile to CharacterId-keyed metadata; label is presentation-only | Cut 4/5 | locale/display-frame tests |
| `character_callee_keys` and suffix candidates | deleted | Cut 4 | same-local-name and qualified alias tests |
| `LineDisplaySpec.callee` | deleted | Cut 5 | display catalog codec/API tests |
| `LineDisplayFrame.callee` | deleted | Cut 5 | native/Web/Agent frame tests |
| `LineDisplayFrame.speaker_label` | replaced by typed `DialoguePresentationCharacter { id, display_name }` | Cut 5 | frame/save/observation tests |
| `BundleViewTextValue::DialogueSpeaker` naming | internal temporary projection removed by 009.4.1; 009.4 wire already uses Character payload | Cut 5 plus 009.4.1 projection cut | cross-package handoff tests |
| `DialogueTextProjection::Speaker` naming | 009.4.1 maps `dialogue.character.display_name`; no identity role retained | 009.4.1 only | follow-up contract gate |
| `RichTextCascadeLayer::SpeakerPreset` | renamed directly to `CharacterDialogueConfig` | Cut 5 | cascade precedence/provenance tests |
| tooling `speaker_line_edit` | replaced by typed colon-to-bracket edit | Cut 6 | exact edit tests |
| tooling `ExactSpeakerRecord` | replaced by exact checked CharacterDialogue application record | Cut 6 | stale/missing/duplicate fact tests |
| canonicalizer output `.say(...)` | direct bracket canonical output | Cut 6 | output behavior tests |
| `.say` completion/hover/signature/code action | none | Cut 6 | negative LSP tests |
| `arcw fmt --expand-sugar`-style semantic rewrite | none; formatter remains syntax-only | Cut 6 | CLI flag/output tests |
| docs treating `.say` as canonical | direct CharacterDialogue source surface | Cut 7 | documentation build/examples |
| samples/fixtures containing canonical `.say` | direct bracket/colon/factory forms | Cut 7 | sample compile/run tests |

## 3. Codec, persistence, and wire residue

| Discarded representation | Final decision | Cut | Rejection proof |
|---|---|---|---|
| AWBC ABI 1 / codec 7 executable for this new runtime model | sole accepted ABI 2 / codec 8 | Cut 4 | unsupported-version and old-byte fixtures |
| anonymous runtime record used for a nominal CharacterDialogue | `RuntimeValue::NominalRecord` with `std.character_dialogue` and exact layout | Cut 1/4 | wrong nominal/layout/field tests |
| any static preset table keyed by local name | no representation | Cut 4 | AWBC inventory and dynamic execution tests |
| unversioned old display-catalog transcript | display catalog schema 2 only | Cut 5 | old transcript missing schema is rejected |
| bundle session save schema 1 | schema 2 only | Cut 5 | restore rejects schema 1 before mutation |
| old dialogue `callee`/speaker fields in observation/debug payloads | typed Character fields only | Cut 5/6 | old JSON/payload rejection tests |
| Speaker/preset-specific root replay record | none ever existed; no reader is added | none | root replay v1 generic payload validation |
| migration that drops or reinterprets stale config fields | prohibited | Cut 5 | stale contract tests reject whole value/candidate |
| dual reader selected by old enum/discriminant | prohibited | all codec cuts | wrong/old discriminant tests |

## 4. Rust public API deletion rules

The deletion cut must update all workspace consumers in the same commit. It
must not leave:

- root-level re-exports of an old type;
- compatibility modules such as `speaker`, `speaker_preset`, or `say`;
- deprecated methods or trait extension methods;
- aliases whose target is `CharacterDialogue`;
- serde aliases, defaulted legacy fields, untagged dual shapes, or versioned
  fallback decoders;
- old enum numeric positions preserved solely for an unreleased shape;
- hidden parser branches accepting `.say` as dialogue syntax;
- a dedicated `.say` error code or recognizer;
- a formatter rewrite from `.say` to brackets;
- source scanning tests that search repository files for forbidden spellings.

Compile-fail tests target the public API and language type checker. Codec tests
feed old/malformed bytes directly. Tooling tests invoke formatter/canonicalizer
APIs. Architecture tests use Cargo metadata and typed dependency graphs.

## 5. Completion gate

The old path is considered deleted only when all `DEL-*` rows in
`TEST_MATRIX.md` pass together with the relevant focused/workspace tests. A
remaining internal symbol is not excused merely because it is private: any
legacy successful behavior, identity reconstruction, old decoder, or alternate
source surface keeps the cut incomplete.
