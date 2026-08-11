# Repository-aware validation

## Result

```text
status: PASSED
contract_agent_validated: true
repository: Sanzentyo/arcweft
ref: main
pinned head: 5821a3ca479b5b89ca6ede997b9cf4f42f6280a6
production code changed: false
fallback: false
open questions: 0
```

“Passed” means the final contract was reconciled against the exact current
repository owners and the package itself passed deterministic integrity and
contract lints. It does not claim production implementation tests that could
not apply to an unchanged, connector-only worktree.

## 1. Revision and rule validation

| Check | Evidence | Result |
|---|---|---|
| Latest `main` selected | recent commit query and exact commit fetch | pass |
| Task noticed `main` advance | initial `126f7ece...` pin was discarded; final pin is `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6` | pass |
| Latest `AGENTS.md` read completely | blob `e91f99213dde67953beda6aa078c370a8dc4541d` | pass |
| Rust skill read completely | local SHA-256 `1A28F552ADF5EFDE95205BEE8D56590AEB82346C48EBDF3FDBBAFF5DECA33665` | pass |
| Project premise applied | local SHA-256 `CFA897A0AD93DEB92FD454079DF0A789EDBBD40D85C8377324DA703C8AEFE0A1` | pass |
| Prior ZIP intake checked | repository ledger and blob `4b9a0303...` | pass |
| Commit status queried | connector returned zero status contexts | recorded, no CI inference |

## 2. Concrete repository reconciliation

### Manifest authority

`arcweft-manifest-model::ContentRootRef` and `ContentUnitSpec` remain the neutral
schema owner. `arcweft-launch::accepted::SourceBackedManifest` is the one strict
decoded/source-backed authority and already exposes exact content-unit/root/
profile source projections.

Contract consequence: roots are collected from accepted manifest data exactly
once. No source reparse or second decoder is introduced.

### Binary topology and Character substrate

`arcweft-project::content` already owns Sans-I/O exact binary resources and the
canonical topology revision. Project-loader already separates text and binary
payloads/overlays, validates exact Character packages, generated metadata,
project containment, and unconsumed binary overlays.

Contract consequence: those decisions are inherited. No new topology tag,
binary representation, directory scan, or `SourceDocument` binary abuse is
introduced.

### Current root-family gap

The current loader still recognizes Character roots with
`strip_prefix("@character.")`. This is a transitional concrete gap against
`AGENTS.md` owner rules and the requested closed typed family.

Contract consequence: final classification moves to the owning typed
entity/resource contracts; the loader consumes a typed Character acquisition
request. No local string match table or extension trait is allowed.

### Source and source-content transition

Current syntax/sema still contains `Item::Source`, `EntityKind::Source`,
`TypeKind::Source`, `EntityDeclKind::Content`, `EntityDeclBody::Content`, and
old source-content ProjectIndex relations. The repository's accepted
Lang-01.3.1 direction explicitly deletes those surfaces and keeps ordinary
`fn -> Stream<T, E>`.

Contract consequence: the final family cannot consume the current transitional
Source enum. It freezes after Source elimination, deletes Source from the family
and target types, makes Stream callables ordinary wrong symbol kinds, and
replaces source-content relations with manifest facts.

### Symbol and visibility authority

The current HIR symbol substrate is revision-bound and already owns callable,
external, nominal, module, visibility, ambiguity, canonical binding, and
bounded linking behavior.

Contract consequence: content admission consumes/extends the authoritative
typed world rather than adding a loader-local resolver. Alias/reexport source
evidence and exact revisions are retained.

### Resource/retained identity authority

`ResourceDeclarationIdentity` already combines exact `EntityId`, `PublicId`, and
nominal `ResourceTypeId`. `ResourceRef<T>`, `AssetRef<P>`, and retained identity
references are distinct typed categories.

Contract consequence: configured roots require an actual accepted declaration;
prefix spelling and Stream return type are insufficient. Typed resource and
metadata references feed the same reference inventory without string scans.

### ProjectIndex and consumers

Current ProjectIndex contains an old source-content `ContentRoot` relation
family. Current loader watch inventory is built from exact topology resources.

Contract consequence: ProjectIndex receives direct manifest unit/root facts and
typed graph endpoints; the old source-content producer is deleted. Bundle,
watch, and LSP consume one accepted inventory and may not rescan.

## 3. Prior Lang-01.5.1.2 baseline validation

Repository intake records:

```text
outer SHA-256:
CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0
```

The baseline ZIP's central directory contained 26 entries. The relevant
`CONTENT_ROOT_FAMILIES.md`, `RUST_SHAPES.md`, and
`REVISION_AND_ADMISSION.md` entries were recovered and compared against the
current repository and request.

The direct conflict is exact:

- baseline accepts a Source root and exposes Source family/target shapes;
- accepted Lang-01.3.1 removes the declaration/type/entity path;
- current intake ledger marks the family portion blocked pending this request.

`NORMATIVE_DELTA_LANG_01_5_1_2.md` resolves that conflict without redesigning
the safe subset.

## 4. Static contract checks

The generated contract was checked for:

- exactly 10 accepted categories: Character, eight authored entity families,
  and exact configured resource;
- no Source accepted category in machine-readable inventory;
- no callable accepted category;
- no old source-named positive family/target API in normative final-shape files;
- no Source-specific migration diagnostic;
- no source-content compatibility node;
- `fallback=false`;
- `OPEN_QUESTIONS=0`;
- no patch/diff/production source entry;
- 160 unique test IDs;
- 55 unique decision IDs;
- exact request inclusion;
- source inventory and validation boundaries.

## 5. Executed artifact validation

The package build executes and records:

- UTF-8/newline normalization;
- path-containment checks;
- duplicate-entry checks;
- forbidden symlink checks;
- deterministic timestamps/permissions/order;
- file manifest generation;
- SHA-256 verification for every covered entry;
- JSON and CSV parse checks;
- state/machine-contract agreement;
- forbidden legacy positive-shape lint;
- no-placeholder lint;
- ZIP `testzip()` and independent extraction/hash validation;
- repeated deterministic ZIP build byte equality.

The exact log is `verification/package_validation.log`.

## 6. Commands not claimed

No local repository checkout was exposed, so no Cargo, rustfmt, clippy, Tier 2,
or repository structural command was newly executed. No production code changed,
so this package does not mislabel that absence as a production test pass.

Production completion requires the command-level tests listed in
`TEST_MATRIX.md` after implementation. The final contract itself is not
blocked/fallback because all requested design decisions are closed and
repository ownership has been reconciled.
