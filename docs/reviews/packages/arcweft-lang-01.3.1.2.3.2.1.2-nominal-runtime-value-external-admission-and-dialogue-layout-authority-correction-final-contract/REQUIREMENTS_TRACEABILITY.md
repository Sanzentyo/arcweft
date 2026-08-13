# Requirements traceability

| Request requirement | Closed decision | Primary sidecar | Tests/inventory |
|---|---|---|---|
| 1. sole layer-correct external construction authority | non-Serde producer/value handles from admitted plan catalog; only handle calls crate-private constructor | `FINAL_CONTRACT.md` D1; `RUST_OWNERS_AND_APIS.md` §§1–4 | NREA-008–015; INV-001–011 |
| 2. canonical layout delivery | compiler/runtime-plan catalog declaration, whole-plan `try_admit`, no reconstruction/copy | `AUTHORITY_AND_CATALOG.md` §§2–6 | NREA-001–007, 030–032; INV-020–024 |
| 3. CharacterDialogue input/ownership correlation | catalogs + closed role/custom types + bound producer capability; no root nominal/layout | `FINAL_CONTRACT.md` D3; API §7 | NREA-033–041; INV-025–027 |
| 4. final root/custom/inline representation | exact opaque tuple18; custom tuple2; direct inline variant | `CHARACTER_DIALOGUE_REPRESENTATION.md` | NREA-033–049; INV-026–031 |
| 5. resolve `Dynamic` | delete it; custom ID selects exact checked type and catalog tree validation | representation §4; final D5 | NREA-042–046; INV-028–029 |
| 6. nested nominal lookup | producer-bound expected type + catalog; typed missing/stale/conflict/wrong-producer | authority §§7–8; final D6 | NREA-016–026, 038–041, 067 |
| 7. normalize/empty/patch | descriptor-aware transform, checked-type clear, all-path preflight, atomic publish | `DESCRIPTOR_LOOKUP_AND_TRANSFORMATION.md` | NREA-053–065; INV-036–041 |
| 8. deserialize/restore/session/root/replay/bundle | raw quarantine -> admitted plan/type/schema validation -> traversal/activation; A4/A6 split | `PERSISTENCE_ACTIVATION_AND_CODEC.md` | NREA-050–052, 066–071; INV-043–053 |
| 9. exact error mapping/path evidence | core typed source chain; dialogue role/custom/patch path; driver/save/replay preserve source | `VALIDATION_ERROR_AND_PATH_PRECEDENCE.md` | NREA-016–026, 035–041, 061–071 |
| 10. deletion inventory/order | exact inventory and G0–G10 schedule; final G8 cut | inventory + `IMPLEMENTATION_ORDER.md` | NREA-013–015, 046, 049–051, 059, 065, 077–080 |
| Required validation precedence | exact universal, root, and patch orders | precedence §§1–3 | NREA-021–026, 035–041, 061–064, 066–071 |
| Required producer/consumer inventory | core/pattern/pure/engine/AWBC/root/replay/ownership/nesting/dialogue/plan/driver/View/bundle/save/agent/CLI/accelerator | inventory | INV-001–056 |
| Required tests | 80-row positive/negative/compile-fail/golden/gate matrix | `TEST_MATRIX.md/.csv` | NREA-001–080 |
| no public raw constructor | handle private; `new`/`validate_shape` deleted | API §§2–4; final D10 | NREA-011–015 |
| no hash/name/schema recovery | catalog emitted from accepted descriptors only | authority §§2–3 | NREA-004–006, 017–020 |
| no Dynamic/optional/fallback/producerless opaque | explicit non-goals and representation | `NON_GOALS.md`; representation | NREA-042–049, 059, 078 |
| versions remain 1 | direct unreleased replacement | final §§1,9; persistence §9 | NREA-076,080 |
| design-only archive | no production files/patches; package validation | `IMPLEMENTATION_STATUS.md`; `VALIDATION_EVIDENCE.md` | archive checks |

`OPEN_QUESTIONS=0`; no requirement is deferred to an external design authority.
