# Request-to-contract traceability

| Request requirement | Closed decision | Primary package evidence |
|---|---|---|
| exact host binding key | typed topology/import/source/target/package/module/artifact/export key; full field coverage and structural comparison | `FINAL_CONTRACT.md` §4, `CANONICALIZATION_AND_CODEC.md`, `RUST_API_SHAPES.md` |
| import identity and mount | import ID + typed mount in key | key docs; M-01/M-02 |
| target family and ABI | typed family/ABI/detail claim plus strict current accepted-marker validation in products | API shapes; K-09/K-10; M-13–M-17 |
| package/module/version | complete `AdapterPackage`/`AdapterModule` | key docs; M-10–M-12 |
| metadata ABI hash | metadata identity field | M-18 |
| artifact path/digest/size | complete accepted `AdapterArtifact` | M-20–M-22 |
| export identity | complete function or Activity export plus selected Activity implementation | M-23–M-35 |
| Activity abstract/implementation/interface/state | invariant abstract/metadata Activity identity + `ActivityImplementationId` + complete metadata export; canonical launch selection | key/product docs; M-32–M-35; P-15 |
| Sans-I/O catalog | new metadata-aware shared crate, generic already-constructed binding slots, no I/O/backend deps | `FINAL_CONTRACT.md` §9, crate delta |
| registration API | ID + complete claimed key + typed binding; fixed validation order; no mutation on error | API shapes; R/M/N rows |
| project exact key without re-decode | unified loader transaction consumes retained objects; metadata decode count zero | topology flow; T-09 |
| plan/launch projection | full key once in selected-profile product; typed ID in plan/function values; exact Activity selection in launch assembly; no-profile is `None` | final contract §§6–8; P-series |
| missing/mismatch point | registration mismatch before mutation; runtime missing at pre-host attempt | errors doc; E/N rows |
| stale/wrong family/ABI/artifact/export no fallback | stale-first and complete mismatch taxonomy; fixed slots only | errors doc; M/S/F rows |
| topology/LSP revision correlation | profile + source-set wire identity; process-local generation lease and no carry-forward | final contract §11; S-series |
| fail-closed tests before success | implementation phases 0–8 precede sentinel phase 9 | implementation order |
| one in-memory success | exact sentinel registration/resolve only, no loading/execution | E-03/E-04 and phase 9 |
| unselected generated module cannot invoke | no requirement/ID projected; unselected | R-07/T-04 |
| no fallback through listed spellings/paths/profile | no APIs; terminal failures | F-01–F-10 |
| serialization round trip, no compatibility reader | strict schema 1, Activity selections, optional product presence, and plan variants; reject noncanonical/legacy | W-01–W-23 |
| no partial Activity/host work | pre-dispatch/pre-start gates with state snapshots | N-01–N-10 |
| no metadata/hash/mount/Activity redesign | retained current admission and projection logic; only unified product output | final contract §7; non-goals |
| keep `arcweft-core` Sans I/O | core stores only typed ID variants; catalog/product live outside core | crate delta |
| typed identities, not strings | existing typed values plus private ABI/transport/ID newtypes | API shapes |
| no fallback/aliases/migration/LKG | mandatory deletion and strict codec | deletion matrix; W/F/S rows |
| final Rust shapes and owning crates | complete shapes and file/crate map | API shapes; crate delta |
| implementation/deletion order | phases and mandatory removal rows | implementation/deletion docs |
| complete matrix | K through T rows | `TEST_MATRIX.md` |
| explicit non-goals | bounded list | `NON_GOALS.md` |
| `OPEN_QUESTIONS=0` | all result-changing decisions closed; sidecar exactly `none` | `OPEN_QUESTIONS.md`, `FINAL_STATUS` |
