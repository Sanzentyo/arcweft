# Closed content-root family after Source elimination

## 1. Typed inventory and ownership

| Authored family/category | Final class | Exact accepted target | Identity owner | Resolution owner | Notes |
|---|---|---|---|---|---|
| `character` | file-backed | `Character { character: CharacterId }` | `arcweft-character::id::CharacterId` | project-loader acquisition; `arcweft-project::content` acceptance | exact `assets/<suffix>.awchar` package |
| `flow` | authored entity | `AuthoredEntity { entity: EntityId, family: Flow }` | `arcweft-id::EntityId` | accepted sema entity resolver | exact Flow declaration |
| `view` | authored entity | `AuthoredEntity { entity: EntityId, family: View }` | `arcweft-id::EntityId` | accepted sema entity resolver | global retained View |
| `action` | authored entity | `AuthoredEntity { entity: EntityId, family: Action }` | `arcweft-id::EntityId` | accepted sema entity resolver | global declaration only |
| `activity` | authored entity | `AuthoredEntity { entity: EntityId, family: Activity }` | `arcweft-id::EntityId` | accepted sema entity resolver | Activity identity, not adapter implementation |
| `asset` | authored entity | `AuthoredEntity { entity: EntityId, family: Asset }` | `arcweft-id::EntityId` | accepted entity/asset resolver | payload bytes remain in asset/resource pipeline |
| `signal` | authored entity | `AuthoredEntity { entity: EntityId, family: Signal }` | `arcweft-id::EntityId` | accepted sema entity resolver | global retained Signal |
| `metric` | authored entity | `AuthoredEntity { entity: EntityId, family: Metric }` | `arcweft-id::EntityId` | accepted sema entity resolver | global retained Metric |
| `layer` | authored entity | `AuthoredEntity { entity: EntityId, family: Layer }` | `arcweft-id::EntityId` | accepted sema entity resolver | global retained Layer, not View-scoped handle |
| exact accepted `res` identity | configured resource | `ConfiguredResource { identity: ResourceDeclarationIdentity }` | `arcweft-resource-model::identity` | accepted resource declaration index | exact declaration and registry required |

`ContentRootFamily` is a semantic classification. It is not inferred from a
Rust type name, callable signature, function name, path prefix alone, or
filesystem layout.

`arcweft-id::RetainedIdentityFamily` remains authoritative for its own retained
identity domain. Content-root admissibility is exposed by inherent behavior on
the owning enum; the loader does not copy that table.

## 2. Removed and invalid categories

| Category | Result |
|---|---|
| former Source declaration/root | no final typed target; ordinary unresolved/wrong-target diagnostic |
| ordinary Stream passthrough callable | wrong symbol category; never a root |
| authored Stream generator | wrong symbol category; never a root |
| external Stream capability operation | wrong symbol category; never a root |
| `entry` | selected separately by profile; wrong root family |
| former source `content` declaration | removed; manifest content unit is not a compatibility entity |
| `choice`, `choice_option` | nested flow products; wrong root family |
| `dialogue_line`, `text` | generated/scoped products; wrong root family |
| `input`, `button`, `style` | scoped/runtime presentation products; wrong root family |
| `scene`, `capture`, `hook` | runtime/tooling products; wrong root family |
| `slot`, `target` | scoped presentation identities; wrong root family |
| presentation target | retained dependency, not a root |
| scroll region | View-scoped retained dependency, not a root |
| old `image`, `voice`, `se`, `bgm`, `audio_bus`, `mixer_snapshot`, `ducking`, `motion`, `rig` source families | accepted only as an exact configured resource declaration; otherwise wrong/unknown |
| proof, nominal type, module, function, extern operation | wrong symbol category |
| unknown family without exact configured declaration | unknown family/target |

There is no Source tombstone variant in any final family enum. The ordinary
resolver may still include the original spelling in an error's authored
reference text because that text is source evidence; it must not classify it as
a surviving Source semantic category.

## 3. Normative resolution precedence

For each accepted-manifest `ContentRootRef` occurrence:

1. Parse the reference once with the ordinary typed entity/public-reference
   parser. Preserve both the full scalar value span and the string selection
   span from `SourceBackedManifest`.
2. Obtain the canonical family/public identity without a loader-local
   `strip_prefix` classifier.
3. If the final typed reference category is Character, reserve it for exact
   file-backed Character acquisition. It never falls through to an authored
   entity or configured-resource lookup.
4. Resolve an authored entity through the one accepted entity/symbol world.
   Exactly one target must exist. Its actual `EntityKind`, declaration
   identity, source revision, and visibility determine acceptance.
5. If no authored entity accepts the identity, consult the one accepted
   configured-resource declaration index. Exactly one
   `ResourceDeclarationIdentity` must resolve, and its resource type must belong
   to the exact accepted registry revision.
6. If a final typed declaration exists but its category is not one of the
   closed root families, reject the ordinary wrong-family/wrong-symbol-kind
   diagnostic with typed candidate evidence.
7. If no final declaration exists, reject the ordinary unknown family/target
   diagnostic selected by the current resolver.
8. Multiple authored or configured declarations for one canonical public
   identity are world-integrity/ambiguity failures. Input order never selects a
   winner.
9. Validate visibility and unit publication policy. A content unit may narrow
   visibility but may not expose a target beyond the target's accepted
   visibility.
10. Bind the target to the exact symbol/resource/topology revisions. A stale
    candidate is rejected before any consumer publication.

## 4. Manifest positions

The only permitted root position is:

```text
content-units.<unit-id>.roots[ordinal]
```

The following are deliberately not root positions:

- `profiles.<profile>.entry`;
- `profiles.<profile>.content.<unit-id>` keys and policy fields;
- external-module import/export declarations;
- configured resource value fields;
- source expressions or declarations;
- Character manifest layer entries.

Configured resource value fields can contain typed references to already
accepted roots. Those are reference facts, not additional root declarations.

## 5. Visibility rule

Resolution is performed in the accepted package/project world, not in a
synthetic public-only namespace.

For an authored target:

- the authoritative declaration/binding owner supplies visibility and exact
  source spans;
- aliases/reexports canonicalize to the original declaration and retain every
  binding site used as evidence;
- an inaccessible declaration is not accepted merely because its public ID
  spelling matches;
- `ManifestVisibility::Private`, `Package`, or `Public` is the content-unit
  publication cap;
- the effective publication is the narrower of unit policy and target
  visibility;
- requesting a broader unit visibility than the target permits is
  `ContentRootVisibilityEscalation`.

For configured resources, equivalent visibility is supplied by the accepted
resource declaration/index owner. Character package visibility is the content
unit's manifest visibility because the package has no source declaration
visibility.

## 6. No Stream-derived category

The resolver must not branch on:

- `TypeKind::Stream`;
- own-scope `yield`;
- external capability ownership;
- runtime Stream origin or instance identity;
- callable effect row;
- function/capability name;
- an attribute or naming convention.

If a callable public path is supplied where a content root is required, the
ordinary typed resolver reports the callable as the actual wrong symbol kind.
No replacement Source entity is created.
