# Requirements traceability

| Requirement | Request decision | Normative closure | Tests | Status |
| --- | --- | --- | --- | --- |
| D-01 | Opaque checked representation | FINAL_CONTRACT §2; RUST_OWNERS §§1-4; OPAQUE_OWNER | CORE-001..024; AWBC-002..016 | closed |
| D-02 | Composite recursion | FINAL_CONTRACT §3; COMPOSITE §§1-3 | CORE-025..052 | closed |
| D-03 | Variant ownership | FINAL_CONTRACT §4; COMPOSITE §§4-6; VARIANT API | VAR-001..014; AWBC-017..025 | closed |
| D-04 | Native acceptance | FINAL_CONTRACT §5; OPAQUE_OWNER §§2-4; ERROR | CORE-008..018; PAR-001..006 | closed |
| D-05 | AWBC | FINAL_CONTRACT §6; AWBC WIRE complete | AWBC-001..034; PAR-007..020 | closed |
| D-06 | Producers | FINAL_CONTRACT §7; PRODUCER contract | PROD-001..019; entry tests | closed |
| D-07 | Type reconciliation | FINAL_CONTRACT §8; PRODUCER §§3-4; ERROR §1 | PROD-001..024 | closed |
| D-08 | RuntimeResolvedVariant | FINAL_CONTRACT §9; VARIANT API complete | VAR-001..014 | closed |
| D-09 | Persistence | FINAL_CONTRACT §10; PERSISTENCE complete | SAVE-001..014 | closed |
| D-10 | A1 order | FINAL_CONTRACT §11; IMPLEMENTATION_ORDER | GATE-001..010 | closed |

## Required inventory and constraints

The producer/consumer list is closed by `PRODUCER_CONSUMER_DELETION_INVENTORY.*`. Native/AWBC equivalence is closed by `NATIVE_AWBC_PARITY_MATRIX.*`. Version and migration requirements are closed by `AWBC_WIRE_AND_VERIFIER.md` and `PERSISTENCE_AND_MIGRATION.md`. Prohibitions are enforced by the package validator and deletion rows.
