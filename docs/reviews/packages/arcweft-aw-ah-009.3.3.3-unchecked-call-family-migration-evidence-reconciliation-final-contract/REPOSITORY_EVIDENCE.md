# Repository evidence

## 1. Inspected identity

| Item | Evidence |
|---|---|
| Repository | `Sanzentyo/arcweft` through the connected GitHub repository |
| Git `main` | `5f33ea20fcde7317332c95324701ed4ea7ab813a` |
| Commit subject | `Request typed static-capacity callee contract` |
| Jujutsu change | `yxvlsqorouqlolxvwtltxltmtqutsxku`, supplied by the dispatch as the matching confirmation identity |
| Root `AGENTS.md` blob | `e91f99213dde67953beda6aa078c370a8dc4541d` |
| Rust skill SHA-256 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |

The complete root `AGENTS.md` and complete Rust skill were read. Searches for
more-specific `AGENTS.md` at the relevant crate/docs paths returned no file at
the inspected Git revision.

The repository connector exposes Git content, not an independent Jujutsu
workspace. The Jujutsu ID above is therefore recorded as dispatch-confirmed
identity, not as a claim that this artifact runtime enumerated a separate
uncommitted JJ diff.

## 2. Consumed inputs and integrity

| Input | SHA-256 | Verification |
|---|---|---|
| request Markdown | `c2a101e93213682b8d05e7f08b2fe58cf8c187e6e6f25129b0513941c2e05b2b` | complete local bytes read |
| AW-AH-009.3.3 ZIP | `9d1f989f5e0e698aeff1098dd7ecee7e01a66616a00a0571ee333a3b1b7ddc78` | ZIP CRC/decompression and 11-member manifest verified |
| AW-AH-009.3.3.1 ZIP | `3d81158eb37f503ef7b0f242a79015ba1ab00e3954a8dae4384f45eaab55b672` | ZIP CRC/decompression and 10 listed-member manifest verified; manifest correctly excluded itself |
| AW-AH-009.3.3.2 ZIP | `c5b6bbf9addb45f2d6ecbdfd8f2abc4d6602f079a847a20db8f26140d53a248f` | ZIP CRC/decompression and 14-member manifest verified |
| implementation note Git blob | `2a2c861eeb059f499712f385881d067b95936d98` | connector content read at inspected commit |

The parent package's all-zero manifest self-entry and the external package's
all-zero self-entry were interpreted according to their own README/manifest
rules. Every non-self member hash matched its extracted bytes.

## 3. Current production source evidence

| Path | Git blob | Observed typed evidence |
|---|---|---|
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `484dc6ad790a1a194cc91293700799024adc411b` | `CallableFamily::ALL: [Self; 23]`; exact order includes `StageMethod`; candidate `family()` is inherent; Curried delegates to base |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | `776142ad69e2de6a47bb5c180739d8c679bd20d7` | typed parameter/passing/presence, unknown-name/spread policies, and validator enum |
| `crates/arcweft-lang-sema/src/callable/schema/families.rs` | `5098b97d73f4325db59e2c39cbf5c36bb6379c10` | exact Drop/Promotion/Speaker `variadic_unchecked`; Stage typed schema; reachable typed schemas in the other families |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | `12ab3bbdca5045d53937c5bd49050c715eb4e103` | `Selected`, `Ambiguous`, `Rejected`, `NonCallable`, `Missing`; clean/recovered/rejected poison; typed argument slots |
| `crates/arcweft-lang-sema/src/checker/expr/registered_call/arguments.rs` | `1b9d0fe0b74b94e719ccff0304ed69970a509287` | unchecked slots are checked without expected type; unresolved `actual == None` does not itself poison; mapping/type/spread failures reject and retain typed diagnostics |
| `crates/arcweft-lang-sema/src/signature/project.rs` | `105bad72b47b4dbf62870c9788a137419fde4559` | signature help projects checker-owned Selected/Ambiguous/Rejected facts; Rejected uses deterministic candidate order; Missing/NonCallable are NotApplicable |
| `crates/arcweft-lang-sema/src/callable/error.rs` | `7f1aef249b4106cd17846a4c62bd3c29ce68795e` | typed diagnostic codes include NoViableSignature, mapping/type/spread errors, and terminal errors |
| `crates/arcweft-lang-sema/src/callable/dialogue.rs` | `f4c59781d9637d9e0c7b9e9f4ea2398a2f78d5de` | current typed Dialogue IDs/callee identities; correction does not invent a surface |
| `docs/implementation/2026-07-21-aw-ah-009-3-semantic-selection-and-resource-accounting.md` | `2a2c861eeb059f499712f385881d067b95936d98` | shared resolver/facts/accounting history; impossible fallback cases are not manufactured; old Dialogue carriers are not restored |

## 4. Findings that determine the correction

1. Current `CallableFamily::ALL` has exactly 23 entries. The parent package was
   written against an earlier 22-family enum; `StageMethod` is now explicit.
2. `DropCallableId::Drop` constructs exactly `variadic_unchecked(Unit, Drop)`.
3. every `PromotionCallableId` constructs exactly `variadic_unchecked`, with
   `Promote`/`PromoteUnchecked -> Promoted` and `Assume -> Unit`;
4. every `SpeakerCallableId` constructs exactly `variadic_unchecked` with
   `SpeakerPreset(Character)` result;
5. `variadic_unchecked` is one optional `RestPositional` `Unchecked` parameter,
   `OpenUnchecked` unknown names, and `Unchecked` spread;
6. the argument checker supplies no expected type to `Unchecked`; when an
   expression recovers with no inferred type, that fact alone leaves poison
   clean;
7. all other 20 families have a reachable current typed rejection through a
   representative member. Some contain unchecked members, but the family is not
   wholly non-rejecting for section-19 evidence;
8. signature projection consumes checker-owned retained candidates and does not
   call a second resolver;
9. Missing, NonCallable, unsupported surface, and terminal errors are typed but
   cannot prove a family validator rejected an authored call.

## 5. Arcweft architectural alignment

The inspected architecture keeps typed AST/HIR/IR boundaries, a Sans-I/O core,
a deterministic VM, and explicit host adapters. The inspected decisions favor
canonical typed semantics and reject migration-only parser branches. The
correction follows those principles by using enum identities, schema accessors,
checker facts, and typed query results rather than spelling scans or synthetic
compatibility paths.

## 6. Validation honesty

Newly performed in this artifact runtime:

- complete request/prerequisite/Rust skill reads;
- complete root `AGENTS.md` read at the inspected Git commit;
- connected-repository inspection of the listed production files and docs;
- outer SHA-256 of every consumed ZIP;
- ZIP CRC/decompression tests;
- extracted member-manifest hash/length validation;
- generated output member hash/length validation;
- generated ZIP decompression, clean extraction equality, member-set, manifest,
  and outside SHA-256 validation.

Not performed or claimed:

- no production Rust/test/manifest/schema/fixture edit;
- no Cargo build, test, Clippy, Tier 2, or structure audit;
- no independent checkout of the dispatch-provided Jujutsu change;
- no claim that historical repository validation was rerun.

These limits do not leave a result-changing design question open: the taxonomy
is closed by the accepted packages and the inspected current Git production
schemas. Implementation readiness is conditional on executing the test and
repository gates specified in `TEST_MATRIX.md` after the test-only evidence is
implemented.
