# Closed content-root family contract

## Resolution precedence

For each canonical `ContentRootRef`:

1. Parse the first family segment with the existing typed entity-reference owner.
2. `character` is reserved for the file-backed Character package path and never falls through to source/resource lookup.
3. A family in the source-owned table is reserved for that source owner and must resolve to exactly one accepted source symbol of that family.
4. For every other canonical family, consult the accepted configured-resource declaration index first. Exactly one actual `ResourceDeclarationIdentity` accepts the root, including a migrated built-in family such as `image` or `voice`.
5. If no configured declaration matches and the family is a known invalid built-in, reject `WrongRootFamily`.
6. If no built-in or configured-resource category matches, reject `UnknownRootFamily`.
7. Multiple configured-resource declarations for one public identity are an accepted-world integrity failure, not a first-wins choice.

Aliases and reexports preserve the root occurrence's manifest source but canonicalize to the original target identity and visibility. Private/package/public visibility is checked through the accepted world; no family-prefix shortcut grants visibility.

## Family table

| Authored family | Class | Accepted target | Notes |
|---|---|---|---|
| `character` | file-backed | exact `CharacterId` and validated `.awchar` package | deterministic `assets/<suffix>.awchar` mapping |
| `flow` | source-owned | exact Flow `EntityId` | valid content/link root |
| `view` | source-owned | exact retained View `EntityId` | consumes existing View owner |
| `action` | source-owned | exact retained Action `EntityId` | global declaration only |
| `activity` | source-owned | exact Activity `EntityId` | abstract/source Activity identity, not adapter artifact |
| `source` | source-owned | exact Source `EntityId` | typed source declaration |
| `asset` | source-owned | exact Asset `EntityId` | asset identity; bytes remain in the existing asset pipeline |
| `signal` | source-owned | exact retained Signal `EntityId` | global declaration only |
| `metric` | source-owned | exact retained Metric `EntityId` | global declaration only |
| `layer` | source-owned | exact retained Layer `EntityId` | global retained layer, not scoped presentation layer handle |
| accepted `res` family | configured resource | exact `ResourceDeclarationIdentity` | requires accepted registry and declaration; raw prefix is insufficient |
| `entry` | invalid | — | selected separately by the launch profile; cannot be a content-unit root |
| `content` | removed/invalid | — | manifest content unit replaces source declaration |
| `choice`, `choice_option` | invalid | — | nested flow products |
| `dialogue_line`, `text` | invalid | — | generated/scoped content products |
| `input`, `button`, `style` | invalid | — | scoped/runtime presentation products |
| `scene`, `capture`, `hook` | invalid | — | runtime/tooling products |
| `slot`, `target` | invalid | — | scoped presentation identities |
| presentation target | invalid | — | scoped/retained dependency, not a content root |
| scroll region | invalid | — | View-scoped identity, not a content root |
| old `image`, `voice`, `se`, `bgm`, `audio_bus`, `mixer_snapshot`, `ducking`, `motion`, `rig` source families | invalid as source-owned | exact configured resource only | accepted only when an actual `res` declaration resolves; otherwise wrong family |
| proof/type/function names | invalid | — | not entity-root identities |
| unknown prefix | unknown | — | no guessing or string fallback |

## Owner behavior

- Add the built-in classification as an inherent method on `arcweft_lang_sema::types::EntityKind`.
- Add exact configured-resource lookup to the accepted resource declaration/index owner.
- Do not duplicate the table in the loader, compiler, LSP, bundle, or tests.
- Do not create an extension trait around `EntityKind` or a stringly helper such as `family_name_to_root_kind`.
