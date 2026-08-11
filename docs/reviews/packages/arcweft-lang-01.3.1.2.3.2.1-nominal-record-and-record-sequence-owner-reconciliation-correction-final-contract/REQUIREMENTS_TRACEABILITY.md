# Requirements traceability

| Request requirement | Closed by |
|---|---|
| 1 exact final owner/name | `FINAL_CONTRACT.md` §§2–3; `RUST_OWNERS_AND_APIS.md` §1 |
| 2 fields/visibility/traits/ctors/accessors/Arc/equality | `RUST_OWNERS_AND_APIS.md` §§1,4–5; `DEPENDENCY_AND_SHARING.md`; `TRAIT_CODEC_AND_PERSISTENCE.md` |
| 3 reconcile value/role/schema/sema/identities | `NOMINAL_LAYOUT_AND_PROJECTION.md` §§1–9; `FINAL_CONTRACT.md` §4 |
| 4 exact `try_from_accepted_layout` checks | `FINAL_CONTRACT.md` §6; `ERROR_AND_PRECEDENCE.md` §3 |
| 5 APIs/errors/unchecked new migration and deletion cut | `RUST_OWNERS_AND_APIS.md` §5; `IMPLEMENTATION_ORDER.md` A4; `COMPILE_FAIL_MATRIX.md` |
| 6 existing vs new sequence error | existing `RuntimeSeqError`; `FINAL_CONTRACT.md` §9 |
| 7 sequence variants and precedence | `RUST_OWNERS_AND_APIS.md` §8; `ERROR_AND_PRECEDENCE.md` §5 |
| 8 one validated carrier/visitor | `VISITOR_AND_CARRIER_CONTRACT.md`; `FINAL_CONTRACT.md` §§8,11 |
| 9 compile-clean trait schedule | `TRAIT_CODEC_AND_PERSISTENCE.md` §§1–2 |
| 10 inventory and G1.2-A continuation | `PRODUCER_CONSUMER_DELETION_INVENTORY.*`; `IMPLEMENTATION_ORDER.md` |
| core value carrier/error inventory | inventory INV-001–020 |
| ownership traversal | INV-021; visitor contract |
| pattern/schema/pure/engine/AWBC/root/replay/nesting/materialization | INV-017–030 |
| entry role/schema | INV-023–024 |
| HIR/sema/runtime-plan producers | INV-031–040 |
| snapshot/bundle/save | INV-030,042–044 |
| contiguous one-based IDs tests | NREC-001,029,034,050–053 |
| initializer reordering | NREC-015–016,053,060 |
| duplicate/missing/extra/arity/column/count/overflow precedence | NREC-003,017–022,030–039,043–049 |
| compile-fail unchecked/raw construction | NREC-067–072; compile-fail matrix |
| canonical identity/byte distinctions | NREC-057–060 |
| visitor path tags/no name fallback | NREC-051–056 |
| no reverse dependency | NREC-065; dependency contract |
| core/workspace check/Clippy at gates | NREC-073–078; implementation order |
| preserve accepted IDs/path/codecs and ABI | final contract §§1,12; trait/codec contract |
| no alias/copy/second enum/side IDs/dual reader/compat ctor | final contract throughout; package validator |
| OPEN_QUESTIONS=0 | `OPEN_QUESTIONS.md`; `FINAL_STATUS.md` |
| no production overlay; all sidecars in ZIP | package validator and archive manifest |
