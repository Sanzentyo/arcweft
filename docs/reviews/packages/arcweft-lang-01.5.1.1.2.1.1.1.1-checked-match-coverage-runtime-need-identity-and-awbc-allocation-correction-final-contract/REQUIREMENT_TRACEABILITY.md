# Required exact decision traceability

| Decision | Subject | Closed resolution | Normative artifact |
|---:|---|---|---|
| 1 | AWBC opcode allocation | exact table 0x00..0x2e and 0x80..0x90; other bytes reject | `AWBC_ALLOCATION_AND_WIRE.md` |
| 2 | numeric owner | repr(u8) AwbcOpcode discriminants + ALL-derived decode table + direct numeric Serde/Wire | `RUST_SCHEMAS.md` |
| 3 | function kinds/flags | exact tags, tombstones, typed private flag set and kind constraints | `AWBC_ALLOCATION_AND_WIRE.md` |
| 4 | integer/wire grammar | shortest varu32, fixed-width exceptions, tensor repair, single final buffer | `AWBC_ALLOCATION_AND_WIRE.md` |
| 5 | NeedId families | fixed BLAKE3 identities for host/View/line/AwaitMany/timeout and direct Await preservation | `NEED_TASK_IDENTITY.md` |
| 6 | AwbcTaskPlan | replace string need_id with mandatory AwbcTaskProducer; no optional fallback | `RUST_SCHEMAS.md` |
| 7 | indexed identities | domain-separated fixed bytes; child index exact u32-le; display hex never parsed | `NEED_TASK_IDENTITY.md` |
| 8 | Task relation | NeedId logical result; TaskKey policy/generation; TaskId launch; deterministic fanout/replay/replacement | `NEED_TASK_IDENTITY.md` |
| 9 | deletion matrix | all current string/suffix/snapshot/codec/bundle consumers replace atomically | `NEED_CONSUMER_DELETION_MATRIX.md` |
| 10 | coverage owner | private MatchCoverageAnalyzer; CheckedMatch constructor accepts no coverage | `CHECKED_MATCH_COVERAGE.md` |
| 11 | coverage algorithm | bounded typed usefulness matrix for every current pattern family | `CHECKED_MATCH_COVERAGE.md` |
| 12 | guard semantics | Absent/ConstantTrue contribute; ConstantFalse/Dynamic do not | `CHECKED_MATCH_COVERAGE.md` |
| 13 | publication | non-exhaustive/unsupported/limit are hard; unreachable warns and is retained after success | `FAILURE_PRECEDENCE_AND_ATOMICITY.md` |
| 14 | coverage tests | required positive/negative/nested/limit/property/differential rows | `TEST_MATRIX.md` |
| 15 | ownership table | every TypeKind maps to Copy/SnapshotClone or a closed rejection | `OWNERSHIP_AND_PERSISTENCE.md` |
| 16 | ownership inputs | ProjectSymbolTable + RegisteredSemanticWorld + ResourceTypeRegistry in final-analysis context | `OWNER_API_MAP.md` |
| 17 | Need type versus producer | Need<T> handle Copy; producer captures/arguments separately certified | `OWNERSHIP_AND_PERSISTENCE.md` |
| 18 | recursive ownership | depth/node/nominal limits, canonical first error, cycles and opaque evidence closed | `OWNERSHIP_AND_PERSISTENCE.md` |
| 19 | stable projection | HIR IDs remain session lookup facts; program/revision/site/arm/output are product coordinates | `CHECKED_MATCH_DIGEST.md` |
| 20 | checked Match digest | exact BLAKE3 transcript over stable semantic digests, coverage, ownership and resource digest | `CHECKED_MATCH_DIGEST.md` |
| 21 | request chain | exact current request copy and predecessor/current hashes in machine/request-chain.json | `VALIDATION.md` |
| 22 | source evidence | baseline-locked numeric line ranges for every affected owner/consumer | `SOURCE_EVIDENCE.md` |
| 23 | constructible APIs | RuntimePlanSemanticFactInput and awbc::vm::step/step_with_host; no invented owners | `OWNER_API_MAP.md` |

Validation fails if any decision is missing, points to a nonexistent artifact,
or contradicts the corresponding machine projection.
