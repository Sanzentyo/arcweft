# Deletion-driven implementation order

## 1. Governing rule

Implement directly against the final authority. Do not first add a parallel checked metadata catalog and migrate later. Each coherent cut begins by deleting or making uncallable the obsolete owner/fallback named for that cut, then fixes compile fallout toward the sole accepted/checked owner. Every cut ends compile-clean and test-clean for its stated scope before the next cut begins.

No cut may introduce a compatibility alias, shim, dual reader, field synchronization pass, source gate, old codec reader, removed-syntax-only diagnostic, string-ID fallback, CSS path, or Takumi path.

## 2. Baseline and scope lock

Before editing:

1. checkout pushed Git `b305c698b22a01b30f1d7e68be6d925e6e3a2875` or a deliberate descendant containing this contract;
2. record `git rev-parse HEAD` and local Jujutsu change ID;
3. read current root/descendant `AGENTS.md` and Rust skill in full;
4. verify the parent/correction ZIP hashes and package precedence;
5. create one clean implementation change; do not revive the isolated pre-correction WIP as authority;
6. run current focused sema/catalog tests to establish baseline; and
7. record existing unrelated failures separately without weakening final tests.

The correction does not reopen ordinary parser precedence, work accounting, `CallableDeclarationId` behavior for existing families, suspension/cancellation classification, DirectFrame/StreamFactory, E017S semantics, CSS, or Takumi.

## 3. Cut A — structural callable identity and original enum owners

### Delete/make uncallable first

- make any public raw constructors that would fabricate new trait/method callable IDs inaccessible;
- remove planned/partial ad hoc trait-method identity helpers from the held WIP;
- prevent new callers from using `ProjectDeclarationId::Callable(CallableDeclarationId)` directly.

### Implement final owner

In `arcweft-lang-hir::symbol::identity`:

- add `TraitDeclarationId`, `ImplDeclarationId`, `TraitMethodRequirementId`, `ImplMethodKind`, `ImplMethodDeclarationId`, and `CallableDeclarationKey` with private fields and owner-specific crate constructors;
- change `ProjectDeclarationId::Callable` to `CallableDeclarationKey`;
- add `Trait` and `Impl` project declaration variants selected by the parent contract;
- extend `CallableDeclarationOwner` in place with trait requirement/implementation/inherent variants;
- update the original inherent methods (`as_str`, runtime/logical/proof predicates) and add `is_method`, `is_dispatch_contract`, and canonical digest tags;
- implement `CallableDeclarationKey::owner`, source/module/package accessors, ordering, and `semantic_digest` on the original type;
- publish exact HIR callable signature/source records for trait/impl methods in one source-order inventory.

### Repair fallout

Update project symbol table/builders and all exact matches. Do not add an extension trait or temporary alias for old `CallableDeclarationId` consumers. Existing callables are wrapped as `CallableDeclarationKey::Existing` at the owner boundary.

### Gate

- HIR/symbol focused tests;
- compile-fail for raw/invalid constructors and method keys;
- `cargo check -p arcweft-lang-hir --all-targets --all-features`;
- strict Clippy for HIR.

## 4. Cut B — accepted record authority extension

### Delete/make uncallable first

- change `CallableRecord::try_new` to `pub(crate)`;
- delete `TraitCallableId` and `CallableCandidateId::TraitMethod`;
- delete `CallableEffectSchema::Project.declared`;
- delete resolver-created empty trait rows and any copied requirement-as-implementation publication;
- remove any WIP `CheckedCallableFacts` fields that own signature/source/access/provider/publication.

This intentionally creates compile failures in catalog/resolver/trait consumers.

### Implement final accepted owner

In `arcweft-lang-sema::callable`:

- add `CallableAccess` to `CallableRecord`;
- extend `CallableCandidateId` in its original enum for project key, detached, environment, and standard identities;
- change `ProjectCallableCatalog::by_declaration` to `BTreeMap<CallableDeclarationKey, Arc<CallableRecord>>`;
- add exact generic `record(&CallableCandidateId)`/family lookups returning the stored Arc reference;
- change effect schema to `Project { declaration }`, `Detached { declaration }`, or `Fixed(EffectRow)`;
- update canonical record/catalog digest encoding;
- register trait requirements, trait implementation methods, and inherent methods through `RegisteredCallableCatalogBuilder::add_project`;
- keep method records out of module value bindings;
- publish environment and standard records as exact Arcs in the same accepted catalog;
- expose `RegisteredTypeCheckEnv::callable_catalog_arc()` as read-only exact Arc access.

### Repair trait storage

Change `TraitMethodRequirement` and `TraitMethodImpl` to identity/body metadata only. Delete copied `FnSignature`, parameter-group, return-type, source, and row fields where the accepted record is authoritative. Resolution returns typed IDs/references.

### Gate

- accepted catalog construction/digest/limit tests;
- exact same-name project/environment/method record tests;
- pointer identity tests for every index;
- resolver focused tests and AW-AH-009.3 callable catalog tests;
- compile-fail for public `CallableRecord` construction and deleted `TraitCallableId`;
- sema crate check/strict Clippy.

## 5. Cut C — checked identity and private transaction

### Delete/make uncallable first

- remove string/source callable effect IDs and local callable-index identity from effect collector APIs;
- remove public row lookup paths that can answer by declaration name;
- remove any partial public checked catalog reader in WIP;
- remove `CheckedCallableEffects::ExternalOrStandard { exposed }` if present.

### Implement final identity

In `arcweft-lang-sema::callable::identity`:

- implement detached/environment/standard checked declaration variants;
- implement project/detached/environment/standard checked contexts including accepted catalog digest/standard version;
- implement owner-specific constructors and typed lookup errors;
- update `CheckedCallableId::semantic_digest` to domain/version v2 and exact encoding;
- retain parent `CheckedClosureId`, `CheckedEffectCallableId`, and conformance identities.

### Implement private builder

- add `CheckedCallableCatalogBuilder` state machine, pending shells, journal, exact Arc checks, and work limits;
- for registered checks, retain the exact accepted catalog Arc and record Arcs;
- for detached checks, construct each record once inside the private builder and publish no other registry;
- create ID-only candidate/source indices;
- use `RecordFixed` for fixed accepted rows;
- run existing body/effect fixed point once; do not add a second body walk;
- create conformances only after signature/substitution/row validation;
- freeze one immutable Arc.

### Gate

- exact context-pair and v2 digest vectors;
- foreign/stale catalog/world/revision/source/standard tests;
- pointer identity test using `Arc::ptr_eq`;
- rollback tests for every journal mutation family;
- fixed-row delegation test proving no row payload in checked facts;
- exact/one-over work limits;
- focused effect/trait tests and strict Clippy.

## 6. Cut D — typed effect and diagnostic authority switch

### Delete/make uncallable first

- delete public declaration row maps from `EffectAnalysisReport`/`TypeCheckReport`;
- delete `callable_executions: Vec<...>` as a separate authority;
- delete generic `AWF-EFX-001` / `UpperBoundExceeded` handling for the accepted cases;
- delete project method-value rejection and resolver-created permissive rows;
- delete synthesized requirement `TraitMethodImpl` rows.

### Repair to checked catalog

- have checker module collection register all pending shells before body inference;
- bind effect graph nodes to `CheckedEffectCallableId`;
- store body contracts/inferred rows and bodyless requirement contracts in checked facts;
- implement `actual_row`/`exposed_row` in the owning inherent impl;
- implement `EffectRow::check_subset` in `effect_row.rs` exactly once;
- store conformance witness/substitution only;
- route direct, bound, curried, static-witness, iterator, and closure calls through exact checked IDs;
- emit parent E015/E016/E022/E023 typed diagnostics/ranges;
- finish `TypeCheckReport` with one `Arc<CheckedCallableCatalog>`.

### Gate

Run the entire retained parent semantic/diagnostic matrix, including E014–E032, E017/E017S disposition, curried methods, static witnesses, inherited requirements, multiple effects, open tails, direct/transitive suspension, and deterministic traces.

## 7. Cut E — project index and relation switch

### Delete/make uncallable first

In `project_index`:

- delete `ProjectCallableSymbol.signature` and `.source`;
- delete copied project/environment effect fields;
- delete name-keyed `CallableSymbol` metadata rows;
- delete `ProjectSemanticIndex::typecheck_env()` reconstruction;
- delete `project_callable(&QualifiedName)` and linear name scan;
- delete `ProjectGraphSymbolRef::Callable(QualifiedName)`;
- delete raw-HIR/name-based call relation construction.

### Implement final index

- retain `Arc::clone(TypeCheckReport::checked_callables())`;
- key `project_callables` by `CallableDeclarationKey`;
- store checked ID, kind, and interface digest only;
- add method kinds and inherent enum behavior on `ProjectCallableKind`;
- retain environment lowering only as `{ checked, lowering }` keyed by `EnvironmentCallableId`;
- use `ProjectGraphSymbolRef::Callable(CheckedCallableId)`;
- construct relations from typed call target/conformance facts;
- validate structural/checked/record identity and catalog Arc at construction.

### Gate

- same-name Function/View/trait/impl/inherent identity tests;
- relation target tests with overloads/aliases/reexports;
- compile-fail for deleted fields/name lookup/ref variant;
- project-index focused tests;
- sema/compiler check and strict Clippy.

## 8. Cut F — compiler and LSP generation sharing

### Delete/make uncallable first

- remove compiler APIs that accept a separately supplied callable/effect catalog in addition to `TypeCheckReport`;
- remove LSP lookups that use callee spelling/raw HIR when accepted semantic data is missing;
- remove LSP caches whose key omits checked catalog generation.

### Implement exact sharing

- `CompiledProject` retains registered world and the report with checked catalog Arc;
- lowerers borrow `report.checked_callables()`;
- `AcceptedProfileCandidate::try_new` validates exact registered catalog Arc, checked generation, standard version, world/revision/source identities, HIR Arc, and overlays;
- accepted LSP environment atomically publishes the compiled/report/index generation;
- declaration cursor uses exact source index; call cursor uses typed call target;
- hover/signature/navigation/diagnostics query retained record/facts;
- stale results are discarded without fallback.

### Gate

- pointer/generation parity across world/report/index/LSP;
- document edit/profile rebuild/close/cancel tests;
- stale snapshot no-result tests;
- hover/signature/navigation/diagnostic parity and UTF-8 ranges;
- LSP strict Clippy.

## 9. Cut G — Agent projection

### Delete/make uncallable first

- delete `owner:name` callable symbol ID construction;
- delete fallback from a graph callable name to a synthetic ID;
- delete project-index Agent signature/effect copies.

### Implement durable projection

- add structural declaration/environment digest methods on original identity owners;
- project exact checked refs to durable IDs only after catalog validation;
- use `CallableInterfaceDigest` as semantic hash;
- keep display names non-authoritative;
- generate function/signature/effect payloads on demand from the same catalog;
- fail the whole payload on missing/stale IDs; no partial graph.

### Gate

- deterministic graph/function payload tests;
- same-name collision tests;
- stale/foreign/rollback no-payload tests;
- absence of checked-context handles in protocol serialization;
- applicable Agent Tier 2 suites.

## 10. Cut H — persistent interface authority

### Delete/make uncallable first

- delete callable signature reconstruction from `HirModule`/`FnSignature` in `interface_public_symbols`;
- delete fabricated `decl:{index}:{tag}` callable identity;
- delete any fallback from missing checked record to HIR/source text;
- remove old `.awbo` interface decoder/schema branch before adding the final writer/reader.

### Implement final private schema

- replace `PublicSymbolObject` with typed flow/callable/declaration objects;
- serialize exact `PersistentCallableDeclaration`, declaration digest, display name, kind, record signature digest, and interface digest;
- change interface facts input to receive accepted `ProjectSemanticIndex`/checked catalog;
- include registered catalog digest, standard version, and interface digest root in stage inputs/key;
- validate structural digest/order on decode;
- increment `AWBO_SCHEMA_VERSION` once; no compatibility decoder.

### Gate

- canonical byte/golden digest tests;
- same-name structural distinction;
- changed signature/effect/access invalidation;
- changed documentation/source revision accepted-catalog invalidation as specified;
- stale/foreign/missing record produces no object;
- compile-fail/visibility for old HIR-only callable builder;
- compiler/project persistent tests and strict Clippy.

## 11. Cut I — runtime trait identity final switch

This cut retains the parent contract and uses the corrected checked digest v2.

### Delete/make uncallable first

- delete local trait/impl/witness/method-string fields from `RuntimeTraitMethodIdentity`;
- delete `RuntimeTraitMethodInventory::by_witness_method` and `(usize, String)` lookup;
- delete compiler `format!("{:?}")`/name identity paths;
- delete runtime parsing/reconstruction of callable IDs.

### Implement final lowering

- implement `RuntimeCallableId::from_checked_digest` on the original type;
- use implementation and optional requirement runtime callable IDs;
- retain typed `RuntimeTraitMethodLoweringIndex` keyed by conformance/inherent IDs;
- sort typed inputs before assigning `RuntimeTraitMethodId`;
- lower iterator evidence directly from conformance IDs;
- update plan schema/fingerprint atomically; no old reader.

### Gate

Run parent P001–P010 and Z010–Z012 rows, runtime plan verification, save/replay/codec tests affected by schema, and deterministic lowering tests.

## 12. Final validation sequence

After all cuts are integrated:

1. `cargo fmt --all -- --check`;
2. focused syntax/HIR/sema/compiler/LSP/runtime tests from `TEST_MATRIX.md`;
3. `cargo check --workspace --all-targets --all-features`;
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
5. `just test-workspace`;
6. applicable repository Tier 2 command(s), including Agent/project semantic-index paths;
7. `cargo +nightly -Zscript tools/structure-audit.rs --root .`;
8. API/trybuild removal tests;
9. `git diff --check`;
10. dependency-direction audit; and
11. final scan using typed/structured evidence only for removed capabilities—do not claim semantic removal by searching source text for names.

A stale slow test is migrated to the final model. The implementation must not preserve an obsolete copy/fallback merely to keep that test unchanged.

## 13. Commit/push boundary

Each cut above is one coherent public-state commit. Never push a commit exposing both old and new public readers. The final authority switch must reach `main` with all old owners/readers deleted and all mandatory gates green. Record exact Git and Jujutsu identities for each pushed cut in implementation ledgers.
