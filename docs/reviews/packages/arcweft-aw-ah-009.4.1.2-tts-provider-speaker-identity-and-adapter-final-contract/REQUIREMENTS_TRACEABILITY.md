# Requirements traceability

The attached request is the sole specification. This table maps every required
decision and output obligation to exact normative documents and test groups.

| Request obligation | Final decision / owner | Normative location | Test coverage |
|---|---|---|---|
| 1. Subsystem and crate ownership | New lower Sans-I/O `arcweft-audio-tts`; host-only executors; small facade | `FINAL_CONTRACT.md` D1; `OWNERSHIP_AND_DEPENDENCY_GRAPH.md` §§1–7 | STR-001–STR-008 |
| Exact dependency graph and prohibited dependencies | Explicit Cargo edges and forbidden graph | `OWNERSHIP_AND_DEPENDENCY_GRAPH.md` §§1,3,5 | STR-001–STR-003 |
| Request/result/error owner versus adapter SDK owner | TTS model crate owns data; host adapter/provider crates own execution/I/O | ownership §§2–4; protocol §§2–10 | ADP-001–ADP-025; STR-001–004 |
| 2. Nominal identities | Exact profile/speaker/provider/binding/key/locale/style/fingerprint/call types | `IDENTITY_AND_MAPPING_MODEL.md` §§1–3 | ID-001–ID-018 |
| Family separation from Character/device/provider key | Nominal constructors; no blanket conversion/fallback | identity §§2–3; final invariants | ID-002–003, ID-008, MAP-015–016 |
| Request ID/TaskKey relationship | No public request ID; fingerprint + `TaskKey::for_tts` | protocol §3 | ID-016–018, RUN-001–003 |
| 3. Character-to-provider mapping | Explicit profile, Character binding, provider binding; exact deterministic algorithms | identity §§4–9 | MAP-001–MAP-025 |
| Multiple providers/locales/profiles, duplicate/conflict rules | Allowed with unique priorities/defaults and strict conflicts | identity §§4–8 | MAP-003–007, MAP-020–025 |
| Missing profile/direct non-dialogue/hot reload | Typed errors; direct profile/speaker calls; compatibility tuple | identity §§6–9; wire §9 | MAP-008–019, RUN-013–016 |
| No identity/string fallback | Explicitly prohibited | final §§1,5; identity §§7–8 | MAP-015–017 |
| 4. Source and typed-resource surface | One `res` descriptor; three ordinary functions; exact names/types/effect | `SOURCE_RESOURCE_AND_MANIFEST_MODEL.md` §§1–2 | SRC-001–SRC-017 |
| No restored resource keyword | Delete Voice declaration family in Lang-01.4 direct switch | source §1; deletion §§1–4 | SRC-005, STR-008 |
| Character argument named/typed correctly | `character: CharacterRef`; logical speaker `tts_speaker` | source §§1–2 | SRC-002, SRC-007–009 |
| 5. Manifest/generated adapter metadata | Sole schema-1 TOML decoder + direct schema-1 metadata extension | source §§3–5 | MAN-001–MAN-023 |
| Provider declaration/capabilities/binding/public config/artifact/ABI/secret ref | Exact records and join | source §§3–6 | MAN-002, MAN-008–020 |
| Source ranges, duplicates, ordering, semantic digests, atomic publication | Exact range tree/order/digests/transaction | source §§3–5; wire §§2–3 | MAN-002–005, MAN-013–016, MAN-021 |
| No second parser/direct filesystem/provider reader | Accepted typed decoder/handles only | source §§3–5; ownership §5 | MAN-023, STR-003, STR-005 |
| 6. Request/result/adapter protocol | Intent → selected request → provider request; complete validated asset | `REQUEST_RESULT_AND_ADAPTER_PROTOCOL.md` §§1–10 | ID-016–018, PRO-001–013, ADP-001–025 |
| Progress/audio/cancel/timeout/retry/error/capability negotiation | Exact types/state machine/policies | protocol §§5–10; wire §6 | PRO-005–013, ADP-005–025 |
| Reuse ordinary function/suspension/Need/Task | Typed template uses existing suspension; finalizes the existing TaskSpec at runtime-driver dispatch before registry/host publication | final D6; protocol §1; implementation Cuts 2,5,6 | SRC-006, RUN-001A–005 |
| Deterministic identity versus nondeterministic provider bytes | Fingerprint fields exact; observed output digest/replay | protocol §§3,6,11 | ID-016–018, RUN-001–008 |
| 7. Wire and version ownership | Manifest/metadata schema 1; AWFB 22/23; AWTP 1; direct replay correction | `WIRE_VERSION_AND_LIMITS.md` §§1–9 | COD-001–016, PRO-001–013, RUN-006–016 |
| Field order/discriminants/budgets/rejections | Exact primitive, payload, enum, and limit tables | wire §§1,3–7,10–11 | COD/PRO groups |
| Save blocker and hot reload compatibility | Existing HostTasks/pins; exact tuple | wire §§8–9 | RUN-009–016 |
| No dual reader/version memorializing sketch | Direct initial final versions only | final D7; wire §§2–9 | MAN-015, COD-015, STR-008 |
| 8. Adapter/capability boundary | Exact capabilities/check order/traits/host I/O ownership | `CAPABILITY_PRIVACY_AND_DIAGNOSTICS.md` §§1–4; protocol §8 | ADP-017–025, STR-002–004 |
| Cancellation/cleanup/concurrency/rate-limit/secrecy | Exact state and limits | capability §§2–4; protocol §§8–10 | ADP-007–020, ADP-022–024 |
| Sans-I/O simulation and test doubles | Scripted executor/manual clock/availability snapshot | capability §9 | PRO-013, ADP group, STR-003 |
| 9. Diagnostics | Stable code/owner/location/fields/redaction table | capability §§7–8 | ID/MAP/MAN/ADP/RUN negative rows |
| Historical `speaker` ordinary unknown diagnostic | No dedicated removed-name diagnostic | capability §7; source §7; deletion §4 | SRC-009–010, MAN-022 |
| 10. Limits and privacy | Exact centralized limit table and classification/projection | wire §10; capability §§5–6 | Boundary rows across ID/SRC/COD/ADP/RUN |
| Provider keys/credentials not public/Agent-visible | Restricted/secret handling and projection allow-list | capability §§3,5–6 | ID-012, COD-014, ADP-022–024, RUN-019–021 |
| Required implementation order | Eight coherent cuts in mandated dependency order | `IMPLEMENTATION_ORDER.md` Cuts 1–8 | Entire matrix; exit gates per cut |
| Required tests | 179 normative rows covering all requested cases | `TEST_MATRIX.md` | All rows |
| Projection non-blocking contract | No TTS lookup/identity inference in dialogue/View | final §4; capability §6 | RUN-017–018, RUN-020 |
| Do not redesign verified substrate without defect | Preserved substrate list; only typed-error concrete defect corrected | final §3; ownership §§3,6–7; deletion §3 | STR-001–008, RUN group |
| No aliases/shims/dual readers/source gates/CSS/Takumi | Explicit prohibition and atomic deletion | final §5; deletion §4; implementation Cut 7 | SRC-005, SRC-009–010, MAN-015, COD-015, STR-008 |
| Required archive contents | All named members at archive root | `README.md`; `MANIFEST.txt` | Package validator |
| External status and SHA-256 sidecars | Generated beside archive | external `.status.txt` and `.sha256` | Package validator |
| `OPEN_QUESTIONS.md` exactly `none` | Exact file bytes, no newline | `OPEN_QUESTIONS.md` | Package validator |
| Ready status only with closed decisions | All decisions exact; status READY | `FINAL_STATUS.md` | Package validator + this table |

## Traceability result

```text
REQUIRED_DECISION_GROUPS=10
CLOSED_DECISION_GROUPS=10
REQUIRED_IMPLEMENTATION_CUTS=8
DEFINED_IMPLEMENTATION_CUTS=8
NORMATIVE_TEST_ROWS=179
OPEN_RESULT_CHANGING_DECISIONS=0
OPEN_QUESTIONS=0
```
