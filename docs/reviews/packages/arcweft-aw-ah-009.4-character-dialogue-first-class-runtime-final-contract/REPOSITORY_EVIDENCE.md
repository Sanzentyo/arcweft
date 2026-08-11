# Repository inspection evidence

## 1. Inspection identity

```text
repository: Sanzentyo/arcweft
default branch: main
inspected revision: f56ed157f8d9070d9d1c607f739d9bd0baa1675d
commit subject: Implement AW-AH-009.3.3.2 typed external binding paths
inspection date: 2026-07-19
```

The GitHub connector reported `main` and the revision above as identical at the
inspection boundary. All repository observations in this package refer to that
revision. The attached request copied to `SOURCE_REQUEST.md` is the sole
normative requirement source; repository documentation was used only as
current implementation and established-substrate evidence.

The repository is private. Inspection used the configured GitHub connector,
not an assumed public mirror. No production file, branch, commit, or pull
request was created.

## 2. Governing repository rules inspected

`AGENTS.md` at the inspected revision was read completely. Relevant durable
rules confirmed by the inspection are:

- preserve the dependency direction
  `syntax -> HIR -> sema -> runtime-plan/verify -> tooling`;
- keep `arcweft-core` and data-format crates Sans I/O;
- prefer typed owner APIs and direct root-cause replacement;
- unreleased internal formats may be replaced without compatibility residue;
- no deprecated aliases, compatibility modules, dual readers, or removed
  syntax acceptance;
- no source gates;
- run focused tests, workspace check/clippy/tests, and structural audit at
  coherent cut points.

The complete local Rust skill was also read before producing Rust-facing type
shapes. This archive contains no Rust implementation.

## 3. Current implementation evidence

| Repository path | Current-main observation | Contract consequence |
|---|---|---|
| `crates/arcweft-character/src/id.rs` | `CharacterId` is a validated `character.*` nominal ID with typed construction, ordering, serde, and compact segments. | Preserve it as the only Character identity owner. |
| `crates/arcweft-character/src/manifest/model.rs` | Character manifests own look/part/variant inventories and deterministic validation. | Reuse manifest-backed look ownership; do not infer from names. |
| `crates/arcweft-dialogue/src/lib.rs` | Public `SpeakerRef`, `SpeakerPreset`, `SayOptions`, `DialogueLineBuilder::say`, and Character helper wrappers remain. | Delete them directly and make `arcweft-dialogue` own the new domain types. |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | Colon speaker lines and string-callee content calls are separate AST shapes; all requested line options are represented. | Replace the split with expression-target `DialogueContentApplicationExpr`; reuse option/content/range substrate. |
| `crates/arcweft-lang-syntax/src/parser/dialogue.rs` and `parser/helpers.rs` | Bracket dialogue parsing, colon line parsing, recovery, and postfix ambiguity substrate already exist. | Reuse delimiter/content parsing; remove speaker-string meaning only. |
| `crates/arcweft-lang-hir/src/model.rs` | `HirDialogue` retains `callee: String` and copied line option fields. | Carry the target expression and checked patch facts instead. |
| `crates/arcweft-lang-hir/src/dialogue_identity.rs` | line identity derives a `DialogueSpeakerSlug` by stripping `.say` and source spellings. | Delete slug/suffix recovery; use source-owner/scope/ordinal IDs. |
| `crates/arcweft-lang-hir/src/lower_ids.rs` | `@say.*` is already a typed line family, but generated IDs contain a speaker slug. | Retain the family and replace only the generated prefix algorithm. |
| `crates/arcweft-lang-sema/src/types.rs` | `TypeKind::Speaker`, `TypeKind::SpeakerPreset`, and speaker-line classification remain. | Replace them with `CharacterDialogueType { Exact | Any }`. |
| `crates/arcweft-lang-sema/src/callable/dialogue.rs` | Character and preset identities both map to the same speaker-line callable family and already carry typed `CharacterId`. | Preserve shared resolver infrastructure, split the final call surfaces, delete old variants. |
| `crates/arcweft-lang-sema/src/callable/schema/families.rs` | the dialogue schema already publishes typed `look`, View, standard fields, open-checked custom arguments, and no spreads. | Reuse schema parameter machinery and dependent Character look typing. |
| `crates/arcweft-lang-sema/src/checker/expr/callable.rs` | direct checker branches turn Character and SpeakerPreset calls into `SpeakerPreset`. | Remove the ad hoc branches and resolve through the shared typed schema. |
| `crates/arcweft-runtime-plan/src/render_text/defaults.rs` | Character/default styles and display labels are indexed by strings/callee candidates. | Compile to CharacterId-keyed defaults and presentation-only labels. |
| `crates/arcweft-runtime-plan/src/render_text/speaker_preset.rs` | let-bound calls become `DialogueSpeakerPreset`; preset chains are reconstructed from local names and `.say` suffixes. | Delete the module/path and lower the real runtime value. |
| `crates/arcweft-runtime-plan/src/render_text/line.rs` | effective style/View/label is currently reconstructed from the callee/preset chain. | Consume typed `CharacterDialogue` plus static content instead. |
| `crates/arcweft-core/src/value.rs` | runtime values support structural records and functions, but a record value carries no nominal type/layout. | Add the generic nominal-record value correction; do not redesign functions. |
| `crates/arcweft-core/src/entry/identity.rs` | `RuntimeNominalTypeId`, `TypeLayoutHash`, and `RuntimeValueDigest` already exist. | Reuse them for nominal layout, canonical equality, and stale detection. |
| `crates/arcweft-core/src/awbc/schema.rs` | current AWBC is ABI 1 / codec 7; nominal AWBC record types and function apply exist. | Atomic ABI 2 / codec 8 switch is concrete and justified. |
| `crates/arcweft-core/src/awbc/fiber.rs` | save/fiber validation traverses runtime values, validates frame types, and enforces nesting depth 64. | Extend traversal/validation for nominal records and retain the established depth. |
| `crates/arcweft-runtime-plan/src/expr.rs` | executable ordinary function values, partial apply, curried apply, and typed lowering evidence already exist. | Preserve the generic function/currying substrate; CharacterDialogue is a separate nominal callable domain value. |
| `crates/arcweft-bundle/src/resource_codec/product_catalog.rs` | display catalog is a compact deterministic `ProductResourceEnvelope` transcript containing `LineDisplayCatalog`; the content catalog is currently empty. | Preserve the envelope/kind and replace the display transcript atomically with schema 2 static dialogue specs. |
| `crates/arcweft-render-text/src/frame.rs` | `LineDisplayFrame` exposes string `callee` and optional `speaker_label`. | Replace them with typed Character identity/presentation name. |
| `crates/arcweft-runtime-driver/src/dialogue.rs` and `view_runtime` | dialogue occurrences already mount persistent authored Views and retain line/stage/reveal identity. | Preserve state transitions and View mounts; change only the typed input/config wire. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | bundle-session save schema is 1 and contains generic AWBC fiber/runtime snapshots. | Switch directly to schema 2 so new nominal values and obligations are the sole accepted save shape. |
| `crates/arcweft-runtime-driver/src/session/replay/model.rs` | root replay schema 1 is generic runtime payload/transition data and has no speaker/preset/dialogue subrecord. | Keep replay schema 1; validate new nominal payloads transitively and invent no parallel dialogue replay wire. |
| `crates/arcweft-tooling/src/canonicalization.rs` and `line_sugar.rs` | semantic speaker records currently drive canonical output to `.say(...)` for Characters. | Replace with exact checked application facts and colon-to-bracket output. |
| `crates/arcweft-lsp` character/callable features | AW-AH-009 shared character registration, source index, callable catalog, and external binding paths are present. | Reuse these exact accepted-world/query identities; no name inference or replacement resolver. |

## 4. Concrete defects that justify change

The inspection found these concrete defects relative to the request:

1. Character identity is lost into source/callee strings after typed
   registration.
2. A configured dialogue is statically reconstructed only for simple lexical
   lets; branch/return/capture/collection/indirect uses have no one value model.
3. `.say` method spelling influences HIR line identity and tooling output.
4. `Speaker` and `SpeakerPreset` duplicate the Character role even though the
   shared resolver already carries `CharacterId` for both.
5. AWBC nominal record identity is not retained by the generic runtime value
   carrier.
6. display and observation wires expose callee/speaker strings instead of a
   typed Character record.
7. current generated line IDs couple source-site identity to a speaker slug,
   which cannot represent a dynamically selected `CharacterDialogue<Any>`.

The final contract changes only boundaries needed to correct those defects.

## 5. Substrate explicitly not redesigned

The inspection found no concrete flaw requiring redesign of:

- Character manifest identity and nominal look inventories;
- AW-AH-009 registration, alias conflict, source-index, signature-help, or
  external binding substrate;
- accepted-HIR/source revision lifecycle;
- ordinary call groups and argument surfaces;
- ordinary function values, generic partial application, currying, closures,
  AWBC function apply, or function save/restore;
- dialogue content grammar, RichText, line plans, cancellation, scoped handles,
  and line `out` result typing;
- persistent authored View mounts;
- typed Stream work;
- native typed View/text rendering;
- root replay v1 generic transition model.

## 6. Verification scope of this archive

Performed:

- exact request byte preservation and SHA-256 recording;
- latest-main connector inspection at the revision above;
- cross-document decision, requirement, deletion, test-ID, UTF-8/LF, manifest,
  and archive integrity verification;
- deterministic ZIP rebuild comparison and CRC/member validation.

Not performed, by design:

- no Rust production implementation;
- no repository checkout mutation;
- no `cargo check`, `clippy`, workspace tests, Tier 2 render/MCP tests, or
  structural audit against a changed checkout.

Those implementation validations are exhaustively specified in
`IMPLEMENTATION_ORDER.md`, `TEST_MATRIX.md`, and
`verification/IMPLEMENTATION_VALIDATION.md`. Their status is **required after
implementation**, not claimed as current evidence.
