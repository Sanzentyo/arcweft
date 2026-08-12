# Requirements traceability

| Request requirement | Normative closure | Primary tests |
|---|---|---|
| 1. exact ID declarations/validation/reservation | `RUST_OWNERS_AND_APIS.md` §§1–2; D01–D04 | EOP-001–EOP-020 |
| 2. adapter declaration/schema key/errors | `RUST_OWNERS_AND_APIS.md` §3; `SCHEMA_2_CODEC_AND_DERIVE.md` §§1,3–5 | adapter-codec rows |
| 3. Rust declaration/schema/derive diagnostics | Rust APIs §4; schema/derive §§2,6 | rust-abi and derive rows |
| 4. header preflight and precedence | schema/derive §§3–5; `ERROR_AND_PRECEDENCE.md` | all precedence rows |
| 5. programmatic manifest / empty types | Rust APIs §4; final contract §3 | rust-abi-model rows |
| 6. sole `AdapterRustType` accessor | Rust APIs §5 | mounted-rust-type rows |
| 7. mandatory accepted fields/preservation | Rust APIs §6; `ACCEPTED_CATALOG_AND_PUBLICATION.md` §§4–5 | catalog/instantiation/substitution rows |
| 8. adapter-sema conversion/typed source errors | Rust APIs §7; publication §§1–3 | adapter-sema rows |
| 9. digest domains/rows/unchanged identities | `DIGEST_AND_GENERATED_SOURCE.md` §§1–6 | digest/invariance rows |
| 10. generated source/escaping/order/source map | digest/generated §7; publication §2 | generated-source rows |
| 11. complete producer/consumer/fixture inventory | `PRODUCER_CONSUMER_DELETION_INVENTORY.*`; `FIXTURE_PRODUCER_CATALOG.md` | G0/G4/G5 rows |
| 12. exact deletion set | `DELETION_SET.md`; implementation G5 | deletion/source-audit rows |
| required error order | `ERROR_AND_PRECEDENCE.md` §1 | all codec/sema precedence rows |
| required implementation order | `IMPLEMENTATION_ORDER.md` G0–G6 | compile/lint/verification rows |
| no redesign of parent substrate | `FINAL_CONTRACT.md` §§1,6,10; `SUPERSESSION_DELTA.md` | invariance/admission rows |
| exact expected archive / no overlay | README, FINAL_STATUS, validator, MANIFEST | local package validation |

All twelve required decision groups are closed. `OPEN_QUESTIONS=0` and
`OPEN_RESULT_CHANGING_DECISIONS=0`.
