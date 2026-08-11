# Requirements traceability

## Required decisions

| Request requirement | Final decision owner | Primary decisions | Tests |
|---|---|---|---|
| exact typed flow/callable source owner | `SOURCE_OWNER_MODEL.md` | D-002–D-009 | TM-009–TM-015, TM-076 |
| package/module/document qualification stage | `SOURCE_OWNER_MODEL.md`, `PROJECT_COLLISION_TRANSACTION.md` | D-002–D-005, D-024–D-025 | TM-013, TM-076–TM-081 |
| source-backed application ExprId and scopes | AW-AH-009.4.2 consumed by `SOURCE_OWNER_MODEL.md` | D-001, D-006, D-012 | TM-014–TM-015, TM-041, TM-082 |
| generated and explicit candidate construction | `LINE_ID_BUILDER.md` | D-012–D-020 | TM-009–TM-041 |
| fixed flow/callable examples | `FINAL_CONTRACT.md`, `SOURCE_OWNER_MODEL.md` | D-007–D-008 | TM-009–TM-012 |
| relative/family-relative/parent traversal | `LINE_ID_BUILDER.md` | D-017–D-019 | TM-027–TM-038 |
| ownerless behavior | `LINE_ID_BUILDER.md` | D-019 | TM-036–TM-038 |
| generated ordinal policy and failure atomicity | `LINE_ID_BUILDER.md` | D-013–D-016 | TM-016–TM-024 |
| one project-wide transactional namespace | `PROJECT_COLLISION_TRANSACTION.md` | D-024–D-034 | TM-042–TM-060 |
| explicit/generated/cross-document collisions | `PROJECT_COLLISION_TRANSACTION.md` | D-026–D-030 | TM-042–TM-049, TM-058 |
| primary/secondary deterministic evidence | `DIAGNOSTIC_MODEL.md` | D-028, D-035–D-037 | TM-042, TM-047–TM-049, TM-084–TM-088 |
| rollback/current accepted project preservation | `PROJECT_COLLISION_TRANSACTION.md`, `LIMITS_INVALIDATION_AND_CACHE.md` | D-031, D-039–D-041 | TM-050–TM-053, TM-077–TM-083 |
| exact diagnostic codes/owners/projection | `DIAGNOSTIC_MODEL.md` | D-035–D-038 | TM-031, TM-034–TM-040, TM-084–TM-090 |
| replace/bypass string-only HirLowerError | `DIAGNOSTIC_MODEL.md`, `MIGRATION_AND_DELETION.md` | D-038, D-044 | TM-089 |
| text-key derivation/provenance/uniqueness | `TEXT_KEY_AND_RENAME.md` | D-021–D-023, D-034 | TM-060–TM-067 |
| line/Character rename independence | `TEXT_KEY_AND_RENAME.md` | D-009, D-042 | TM-068–TM-074 |
| limits and checked arithmetic | `LIMITS_INVALIDATION_AND_CACHE.md` | D-010, D-016, D-030–D-031 | TM-004–TM-007, TM-020, TM-054–TM-056, TM-065, TM-090 |
| deterministic sort, cache, invalidation | `LIMITS_INVALIDATION_AND_CACHE.md` | D-027, D-039–D-041 | TM-048–TM-049, TM-077–TM-083, TM-093 |
| public/crate/Serde/persistence boundary | all model documents | D-010–D-011, D-033, D-043 | TM-001–TM-009, TM-091–TM-095 |
| direct migration/deletion | `MIGRATION_AND_DELETION.md` | D-044–D-046 | TM-095–TM-100 |
| compiling/package-frontier order | `IMPLEMENTATION_HANDOFF.md` | D-045 | command grouping and all rows |

## Mandatory test groups

| Request group | Matrix coverage |
|---|---|
| owner and generation | TM-009–TM-026 |
| explicit and relative IDs | TM-027–TM-041 |
| collision transaction | TM-042–TM-060 |
| text keys and rename | TM-061–TM-075 |
| accepted lifecycle and diagnostics | TM-076–TM-093 |
| direct deletion, dependency, quality, Tier 2 | TM-094–TM-100 |

## Constraints

| Prohibition | Contract enforcement |
|---|---|
| no Cut 1 or AW-AH-009.4.2 redesign | precedence D-001/D-046 and preserved-substrate sections |
| no second package/module/callable/source identity | D-002–D-006, D-024 |
| no second HIR project or accepted line publication | D-024–D-025, D-041 |
| no compatibility alias, dual reader, deprecated helper | D-044–D-046 and deletion gate |
| no source gate or spelling scan | every test matrix rule and structural audit contract |
| no generated skip/reservation/partial facts | D-015, D-029–D-031 |
| no CSS/Takumi/`.say` route | D-046, TM-098–TM-100 |
| no runtime wire/View/TTS/text-layout decision | explicit non-goals in final and reconciliation documents |

## Archive requirements

The exact 17 required members are present. `OPEN_QUESTIONS.md` is exactly
`none\n`. `MANIFEST.txt` is lexically sorted and uses a 64-zero self-entry.
Summary, machine status, and SHA-256 sidecars are generated beside the ZIP.
