# Identity, visibility, and references

## 1. Owned identity types

The owning vocabulary remains `arcweft_id::RetainedIdentityFamily` with exactly:

```rust
Asset, Character, View, Action, Activity, Signal, Metric, Layer
```

The enum owns, through inherent methods:

```rust
pub const fn prefix(self) -> &'static str;
pub fn from_prefix(prefix: &str) -> Option<Self>;
pub fn validate_public_id(self, id: &PublicId) -> Result<(), RetainedIdentityError>;
pub fn derive_public_id(
    self,
    name: &DeclarationName,
) -> Result<PublicId, PublicIdFamilyError>;
```

`from_prefix` is added to the original enum implementation. Family parsers and Layer reference checks must call it rather than maintaining independent string matches.

A dedicated `AssetId` newtype owns catalog-ID construction and validation. Its context-free conversion from a validated normalized `AssetVirtualPath` is an inherent constructor or `TryFrom<&AssetVirtualPath>` implementation. CLI-local `bundle_asset_id_from_virtual_path` and component helpers are deleted after all callers move.

## 2. Declaration names and public IDs

- `DeclarationName` owns one ordinary local identifier. It is case-sensitive and is not a `PublicId` segment list, display label, or Character alias.
- An explicit retained declaration ID is optional. It must be a plain absolute entity token whose stored body parses as `PublicId` and whose family is the declaration family.
- A missing explicit ID is not a missing identity: `RetainedIdentityFamily::derive_public_id(name)` creates `family.<name>`.
- Private declarations receive the same stable semantic ID as public declarations.
- An explicit ID may contain more segments than the local name and is independent of module path. Renaming the local tooling symbol does not silently rewrite an explicit ID; the rename operation offers an explicit coordinated public-ID edit.
- A derived ID tracks a declaration-name rename because its source is the name. The LSP rename transaction updates all checked references through the project symbol, then recomputes the derived ID and collision-checks before commit.

## 3. Asset identity

An asset has no declaration name. Its semantic `AssetId` is derived from the normalized asset virtual path by the exact algorithm in `FAMILY_GRAMMARS.md`. The catalog retains both the original normalized virtual path and the derived ID.

The following pairs collide and are rejected with both paths attached:

- `images/Hero.png` and `images/hero.webp`;
- `ui/main-menu.png` and `ui/main_menu.jpg`;
- any same-stem paths that differ only by final extension.

The package/build owner records bytes, `BundleDigest`, media/format metadata, decoded format metadata where applicable, and dependency/inclusion provenance. These are not inferred from source declarations.

## 4. Visibility

| Source | Semantic visibility |
|---|---|
| omitted | current module only |
| `pub(super)` | parent module and its permitted descendants |
| `pub(crate)` | current Arcweft package only |
| `pub` | package export surface and legal re-export |

`pub(in ...)` and every other restricted form are rejected by the common visibility grammar. Visibility syntax has its own attached node and range. Visibility never changes the `PublicId` or catalog ID.

Assets have no source visibility. Asset availability is determined by package inclusion/profile admission. An excluded asset is absent from the project catalog and therefore unresolved; it is not a private source symbol.

## 5. Symbol and collision authority

One `ProjectSymbolTable` is authoritative.

- Every source declaration contributes one module-local symbol and one global semantic `PublicId` entry.
- All top-level local names in one module share one namespace. A Character, View, Action, Activity, Signal, Metric, Layer, `res`, ordinary callable, nominal type, or other top-level declaration cannot reuse an existing local name.
- Every semantic `PublicId` is project-wide unique. Wrong-family declaration IDs are rejected before registration; exact duplicates report both source owners.
- Assets contribute catalog symbols to the same project identity lookup, but no module-local declaration name and no HIR `ItemId`.
- View callable and Action channel/callable facets refer back to the same retained source symbol and HIR item. Facet registration does not create a second collision table.
- Character surface alias collision continues through the accepted Character registration owner and reports both declarations. The alias is not imported or re-exported as a top-level name.
- No retained declaration supports signature overloading.

## 6. Imports, re-exports, and aliases

- `use` and re-export select an existing project symbol identity. They do not clone a declaration, allocate a second `PublicId`, or change family.
- A `use ... as ...` alias is a local import binding only. It is not stored in the retained declaration header.
- `pub use` may only expose symbols whose visibility permits it. Re-export collision is a normal local project-symbol collision.
- Checked references and LSP rename operate on resolved symbol identity, not text equality. Mention/soft documentation references retain the existing reference-level policy.
- No removed alias registry is kept after a rename.

## 7. Reference syntax

Declaration IDs are deliberately stricter than value references.

### Declaration position

Accepted: plain absolute `@character.alice`, `@view.MainDialogue`, and equivalent family-correct IDs.  
Rejected: `@.alice`, `@character:.alice`, `@{module}:...`, slash-bearing, brace-bearing, origin-qualified, and wrong-family forms.

### Typed value/member position

The common `EntityRefSyntax` remains authoritative and may represent:

- an absolute stored public ID, such as `@view.MainDialogue`;
- a family-relative value reference, such as `@view:.SideDialogue` or `@asset:.bg.room`;
- an imported/project-qualified form already accepted by the common reference resolver.

Bare `@.suffix` is an ID-scope form and is not a retained value reference. Family-relative value references always state the family anchor.

A family-constrained member stores a typed `RetainedReference` containing the source syntax plus `expected_family`. For an absolute reference, syntax may emit `WrongFamilyReference` immediately. For a family-relative or imported reference, sema resolves through the one project table and emits the wrong-family diagnostic if the resolved symbol differs. It never reparses the token string in a family consumer.

## 8. Source ranges and diagnostics

- Explicit ID diagnostics use the exact entity-reference token range.
- Wrong-family declaration ID diagnostics attach the declaration keyword as a related range.
- Derived IDs use the name-node range as source evidence and are marked derived in tooling.
- Duplicate semantic ID diagnostics use the duplicate declaration/catalog path as primary and the first owner as related.
- Duplicate local names use the duplicate name node as primary and the first name node as related.
- Unknown/inaccessible references use the exact reference node range and retain resolver cause (`unknown`, `ambiguous`, `inaccessible`, `detached`, `wrong-family`, or upstream poison).
- No diagnostic is constructed from a searched substring or a copied display string.

## 9. Identity domains that must remain distinct

| Domain | Example owner | Persisted? | Rename behavior |
|---|---|---:|---|
| Semantic public ID | `PublicId`, `AssetId` | yes where product format requires it | explicit/derived policy above |
| Project symbol identity | package/module/symbol table key | compiler session/project snapshot | re-resolved across snapshot |
| Character surface alias | accepted Character registry | product/runtime data as already defined | dedicated alias rename/collision |
| Authored display label | Character `display_name` expression | evaluated product data | ordinary expression edit |
| Dense View/runtime index | View registry/catalog | product-local, not source identity | rebuilt deterministically |
| Syntax node ID | syntax database/lineage/snapshot/node | no | reconciliation-owned |
| HIR item/member ID | HIR database/module/revision/slot | no | liveness/revision-owned |
| Asset virtual path | project/bundle virtual-file inventory | bundle data | catalog rebuild and collision check |
| Content digest | existing bundle digest owner | bundle/release data | changes with bytes |
