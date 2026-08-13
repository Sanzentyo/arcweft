# Requirements traceability

| Requirement | Decision | Normative location | Status |
|---|---|---|---|
| 1 | non-circular producer payload declaration | FINAL_CONTRACT §2; RUST_OWNERS_AND_APIS §§7-8 | closed |
| 2 | root vocabulary/traversal/limits/order | PRODUCER_ROOT_CONTRACT_AND_TRAVERSAL; CANONICAL_GRAMMARS §§4-8 | closed |
| 3 | compiler/runtime-plan canonical root projection | FINAL_CONTRACT §4; RUST_OWNERS_AND_APIS §11 | closed |
| 4 | all seven CharacterDialogue role types | FINAL_CONTRACT §5; CHARACTER_DIALOGUE_ROLE_AND_CUSTOM_CONTRACT §§2-3 | closed |
| 5 | role construction boundary | FINAL_CONTRACT §6; RUST_OWNERS_AND_APIS §15 | closed |
| 6 | custom declaration/digest grammar | FINAL_CONTRACT §7; CANONICAL_GRAMMARS §7 | closed |
| 7 | voice/Variant/Choice exact grammar | FINAL_CONTRACT §8; CHARACTER_DIALOGUE_VOICE_AND_BRANCH_GRAMMAR | closed |
| 8 | generation identity/correlation | FINAL_CONTRACT §9; GENERATION_IDENTITY_AND_CORRELATION | closed |
| 9 | RuntimePlan fields/try_admit | FINAL_CONTRACT §10; RUNTIME_PLAN_ADMISSION | closed |
| 10 | AWBC serialized equivalent/canonical bytes | FINAL_CONTRACT §11; AWBC_PRODUCT_ADMISSION_AND_CODEC §§2-3 | closed |
| 11 | non-Serde admitted AWBC/product wrapper | FINAL_CONTRACT §12; RUST_OWNERS_AND_APIS §14 | closed |
| 12 | direct raw execution API inventory/closure | FINAL_CONTRACT §13; EXECUTION_API_MIGRATION; inventory CSV | closed |
| 13 | CharacterDialogue schema from aggregate | FINAL_CONTRACT §14; CHARACTER_DIALOGUE_ROLE_AND_CUSTOM_CONTRACT §8 | closed |
| 14 | typed error mapping/path evidence | FINAL_CONTRACT §15; ERROR_AND_PRECEDENCE | closed |
| 15 | ID-only producer lookup decision | FINAL_CONTRACT §16; RUST_OWNERS_AND_APIS §12 | closed |
| 16 | deletion set/compile-clean order | FINAL_CONTRACT §17; IMPLEMENTATION_ORDER | closed |
| P1 | plan/product required precedence | ERROR_AND_PRECEDENCE §§2-5; plan/AWBC docs | closed |
| P2 | producer lookup/schema precedence | ERROR_AND_PRECEDENCE §6 | closed |
| I | required producer/consumer inventory | PRODUCER_CONSUMER_DELETION_INVENTORY.md/.csv | 154 rows |
| T | required positive/negative tests | TEST_MATRIX.md/.csv | 320 rows |
| V | all Arcweft versions remain 1 | README; CANONICAL_GRAMMARS; contract.json | closed |
| O | OPEN_QUESTIONS=0 | OPEN_QUESTIONS.txt | closed |
