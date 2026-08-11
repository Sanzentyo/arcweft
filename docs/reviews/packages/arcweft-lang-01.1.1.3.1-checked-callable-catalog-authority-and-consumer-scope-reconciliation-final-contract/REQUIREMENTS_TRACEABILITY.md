# Requirements traceability

## 1. Correction request decisions

| Request requirement | Closed decision | Normative location | Test evidence |
|---|---|---|---|
| select one callable-record authority | retain `RegisteredCallableCatalog`; checked facts retain exact `Arc<CallableRecord>` | Final §§2,5-6; Catalog §§1-3 | C001-C013, C043-C044 |
| no copied signature/source/docs/access/provenance | removed from `CheckedCallableFacts`, project symbols, trait records, Agent/index records | Final §§5-6,8-9; Identity §§5,10 | C011,C023-C024,C034,C043-C047 |
| exact Rust-shaped fields/constructors/visibility/crate owner | private record/fact fields, crate-private builders, owner-specific IDs, original enum impls | Final §§3-6; Catalog §§2-3; Identity §§2-4 | I001-I018,C013-C014,C040,C050 |
| accepted project/environment registration | one builder, one immutable catalog, same exact record Arcs | Final §§5,7; Catalog §2 | C001-C006,C041-C043 |
| pending checked shells | private builder state; no public reader | Catalog §3; Implementation Cut C | C019-C020, API visibility |
| body/effect inference and trait conformance | existing fixed point; catalog facts/conformance only | Final §§6-7; Catalog §4 | E014-E032,T001-T011,C008-C009 |
| final immutable publication | consuming `finish -> Arc<CheckedCallableCatalog>` after full validation | Final §7; Catalog §§3.6,5 | C020-C022,C048-C049 |
| `TypeCheckReport` construction | one checked catalog Arc; old public row/execution authorities removed | Final §8.1 | C021,C045,C048 |
| `ProjectSemanticIndex` construction | same Arc, structural key + checked ID, no copies | Final §8.2; Identity §5 | C021,C023-C028,C047-C048 |
| compiler/LSP/Agent/persistence consumption | borrow same Arc or durable projection; no fallback | Final §9; Identity §§6-10 | C022,C029-C040,C048-C049 |
| exact Arc sharing | pointer equality registered record/catalog and report/index | Catalog §§3.3,5 | C001-C007,C010,C021-C022 |
| stale source/project revisions fail closed | typed generation/source errors; no output | Final §§4,10; Identity §11 | I012-I018,C015-C019,C030,C049 |
| rollback removes all state | journal covers shells/indices/edges/conformance/closures/rows | Final §7; Catalog §6 | I014-I015,C019,C049 |
| preserve structural and checked identity | structural map key + exact revision-bound checked join | Final §§3-4,8; Identity §§1-5 | I001-I018,C023,C026,C028 |
| no dual resolver | typed target facts/catalog only; name/raw-HIR lookup deleted | Final §§8-10; Identity §§5-6,12 | R001-R013,C025-C030,C037,C047 |
| final `ProjectCallableSymbol` types | structural declaration, checked ID, kind, interface digest | Final §8.2; Identity §5.1 | C023-C025 |
| final `ProjectGraphSymbolRef` types | `Entity(PublicId)` or `Callable(CheckedCallableId)` | Final §8.2; Identity §5.3 | C026-C027 |
| final project relation identity | typed checked refs from call target/conformance facts | Final §8.2; Identity §§5.3,6.2 | C026-C030 |
| durable Agent graph identity | structural declaration/environment digest; name display-only | Final §9.2; Identity §7 | C028,C031-C034,C041 |
| LSP source location resolution | source index or call target -> checked ID -> exact record | Final §9.1; Identity §6 | D008-D011,C029-C030 |
| persistent structural identity | typed structural declaration object/digest | Final §9.3; Identity §8 | C035-C039 |
| durable signature digest | exact accepted record schema digest | Final §9.3; Identity §8.2 | C035,C038 |
| no transient checked handle in persistence | explicitly prohibited | Final §9.3; Identity §8.2 | C036 |
| no consumer effect-row copy | only checked facts or record fixed row own row | Final §§5.3,6; Catalog §§1,4 | C008-C009,C012,C023-C024,C034-C036,C045-C046 |
| registered project/environment consumer rows | exact accepted record owner and candidate IDs | Identity §10 consumer table | C001-C006,C041-C043 |
| ordinary Function/View consumer rows | structural project symbol + checked ID | Identity §§5,10 | C002,C023,C028 |
| trait requirement/impl/inherent consumers | exact record + checked ID/conformance | Final §5.4; Identity §10 | C003-C004,C028,C046,T001-T010S |
| direct/bound call facts | checked target/conformance IDs only | Identity §6.2 | R001-R013,C026 |
| effect graph/conformance | typed endpoints and catalog map | Catalog §4; Identity §10 | T001-T008,C019,C045 |
| accepted LSP hover/signature/navigation/diagnostics | same catalog generation and record | Final §9.1 | D007-D011,C022,C029-C030,C043 |
| Agent graph/function payloads | structural ID/interface digest, on-demand projection | Final §9.2 | C031-C034,C041 |
| compiler `InterfaceSummary` | typed structural object + accepted digests | Final §9.3 | C035-C039 |
| runtime trait lowering | parent typed conformance/checked digest projection | Final §9.4; Identity §9 | P001-P010,C040 |
| deletion-first public switch | Cuts A-I delete old owners before repair | Implementation §§3-11 | Z001-Z012,C011-C014,C024-C027,C039,C045-C047 |
| preserve parent deletion inventory | explicitly retained | Final §1; Implementation Cuts B,D,I | Z001-Z012 |
| no compatibility/fallback/source gate | explicit prohibition | Final §11; Implementation §§1,12 | Z007-Z009,C049-C050 |
| complete validation | focused, workspace, strict Clippy, test-workspace, Tier 2, audit | Test Matrix; Implementation §12 | all rows and mandatory commands |
| removal evidence not source-text search | typed/behavior/codec/metadata/audit only | Test Matrix final rule | Z009,C050 |

## 2. Parent Lang-01.1.1.3 requirements retained

| Parent decision | Status in this correction | Normative location | Test rows |
|---|---|---|---|
| sole checked effect authority | retained, metadata ownership corrected | Final §§2,6; Catalog §4 | E014-E032,C008-C009,C045 |
| actual vs exposed row | retained | Final §6.2; Catalog §4.1 | E014-E032,T004-T008,R003-R008 |
| one source-backed contract clause model | retained | Final §1 | S001-S008 |
| no sema source reparse | retained | Final §§1,9.1 | S004-S006,D008-D009 |
| existing typed row/tail/substitution model | retained | Final §1 | E024-E030,T005-T008 |
| omitted bodyless requirement closed empty | retained | Final §6.2 | E015,D006 |
| authored bodyless impl rejected | retained | Final §1 | T010 |
| programmatic standard method fixed row | retained, now `RecordFixed` delegation | Final §§5.3,6.2 | T010S,C006,C008 |
| inherited original requirement identity | retained | Final §5.4 | T001-T004 |
| exact conformance/substitution | retained | Final §6; Catalog §4.2 | T001-T008,R001-R005 |
| method values/curry semantics | retained | Final §1 | R004-R013 |
| typed effect diagnostics and ranges | retained | Final §1 | E015,E016,E022,E023,D001-D009 |
| CLI/LSP one typed diagnostic | retained | Final §9.1 | D007-D011 |
| E017 superseded | retained | Final §1 | E017,X001-X002 |
| E017S static witness | retained | Final §1 | E017S,R003,R005 |
| one-way runtime callable projection | retained, digest domain corrected to v2 | Final §§4.3,9.4 | P001-P010,C040 |
| vector-only runtime inventory | retained | Final §9.4 | P005-P010,Z010-Z012 |

## 3. Parent clauses explicitly superseded

| Parent artifact clause | Why it is superseded | Final replacement | Direct evidence |
|---|---|---|---|
| `CheckedCallableFacts.signature` | duplicates accepted record schema | retained `Arc<CallableRecord>` | C001-C006,C011 |
| `CheckedCallableFacts.source` | duplicates exact accepted source | record delegation | C001-C006,C011,C029 |
| `CheckedCallableFacts.access` | duplicates declaration access | `CallableRecord::access` | C011,C042-C044 |
| `ExternalOrStandard { exposed }` | copies fixed row | payload-free `RecordFixed` | C008,C012,T010S |
| `CallableEffectSchema::Project { declared }` | copies source row before checked authority | ID-only schema | C009,C014,Z004 |
| trait method copied signature fields | second signature authority | checked ID + exact record | C003-C004,C046 |
| `ProjectCallableSymbol` signature/source retention | project index becomes another authority | IDs/kind/interface digest only | C023-C025 |
| callable graph by `QualifiedName` | spelling fallback and collision | checked ID graph ref | C026-C028 |
| Agent `owner:name` ID | not structurally unique | structural digest | C028,C031-C032 |
| HIR-built persistent callable signature | second signature owner/fallback | record-derived digests | C035-C039 |
| no environment checked declaration | cannot bind accepted environment record exactly | `CheckedCallableDeclaration::Environment` | C005,C032,C041 |
| checked digest v1 without catalog digest | cannot reject foreign accepted catalog generation | v2 context encoding | C015-C017,C040 |

## 4. Constraints not reopened

The implementation must not use this package to redesign:

- ordinary call parsing;
- resolver precedence or work accounting;
- existing `CallableDeclarationId` semantics for existing families beyond wrapping in `CallableDeclarationKey::Existing`;
- accepted nominal world rules;
- direct suspension/cancellation;
- `DirectFrame` / `StreamFactory` classification;
- dynamic trait objects;
- CSS or Takumi;
- source top-level gates; or
- compatibility migration policy.

Test rows R011-R013, Z007-Z009, and C050 protect these boundaries.
