# Source and repository evidence

## Inspected revision

- Repository: `Sanzentyo/arcweft`
- Full inspected `origin/main`: `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`
- Request-stated earlier production SHA: `cbf0acedb98de260d8ecaab70a39933c39f30708`
- Retrieval method: authenticated GitHub connector, because the repository is
  private in the project context
- Local production checkout/patch: none
- Production Rust build/test executed for this design return: none

The latest inspected main includes the predecessor intake documentation. The
design uses current source and that intake rather than repeating the request's
older source observations.

## Exact evidence rows

| Path | Lines | Git blob | Owner | Concrete observation | Verification |
|---|---:|---|---|---|---|
| `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction.md` | `1-548` | `7ed008dec6eddb820e228ea0803bf97a1ead2c36` | current correction request | Exact mandatory decisions A-F, five compile-clean cuts, required tests/artifacts, exact ZIP name. | GitHub connector at inspected SHA; byte-equal to inputs/CURRENT_REQUEST.md by Git blob and SHA-256 |
| `docs/implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-checked-match-need-identity-return-intake.md` | `1-260` | `aaf81e16d2b566c411ed97804c86f860f87bf7c9` | repository intake/reconciliation | Predecessor archive integrity passed but repository reconciliation failed and classified DESIGN_NOT_READY; records nonnumeric contradictions. | GitHub connector full file |
| `crates/arcweft-core/src/task.rs` | `1-360` | `130256a8a8efb2fe6c7028c68357cb707f975eb9` | current task/Need data | Current NeedId/TaskKey/TaskId are String-backed; TaskSpec caller supplies id/key; TaskEvent and RuntimeNeedState lack final complete correlation. | GitHub connector inspected definitions and consumers |
| `crates/arcweft-runtime-driver/src/task.rs` | `1-250` | `22aabea946be7645afcebe65415bcea3cd786eb9` | runtime task journal/dispatch | Consumes driver-local GenerationId and current partial TaskEvent/TaskSpec shapes; uses BTreeMap task records and host dispatch envelopes. | GitHub connector full relevant range |
| `crates/arcweft-runtime-driver/src/swap.rs` | `1-180` | `39ecdfad33abfe4a55e20f8e7501878152ef797e` | current generation owner | Defines runtime-driver-local GenerationId(pub u64), proving the required move to core and duplicate deletion. | GitHub connector range including lines 20-22 |
| `crates/arcweft-runtime-driver/src/generation_runtime.rs` | `1-140` | `22ec3c7d02908333fb44c5990061feeaff736ed8` | generation runtime consumers | Generation-local runtime image code consumes the swap-owned generation type and must import the final core owner. | GitHub connector |
| `crates/arcweft-view/src/view/identity.rs` | `20-223` | `153fa4450cc676e7f6b860c9700dfc7bb95e4315` | current View identity | Current stable owner is ViewProgramId; current accepted semantic revision is AcceptedViewProgramRevision([u8;32]) with validation. No ViewProgramSemanticDigest/u32 revision. | GitHub connector full identity range |
| `crates/arcweft-lang-sema/src/registration/environment_input.rs` | `190-409` | `0f1bb0d1d2b2221e60bb564550cf6d9fb4e9af7a` | accepted nominal inventory input | AcceptedNominalInventoryInput currently carries runtime_producer but no value_class or persistence, making predecessor ownership evidence unconstructible. | GitHub connector constructor/field range |
| `crates/arcweft-lang-sema/src/env/nominal.rs` | `1-260` | `820da68e99b0971d4ceb195eb46436e7aaf6d869` | accepted nominal semantics/catalog | AcceptedNominalSemantics::Opaque currently contains only producer; AcceptedNominalCatalogDigest is the accepted catalog root. | GitHub connector |
| `crates/arcweft-lang-sema/src/types.rs` | `300-700` | `528d32482865aae4bf8dd8f456468b061ecdedc7` | current TypeKind | Enumerates the exact current semantic type variants reconciled by the total ownership matrix, including Need, Ref, Stream, Function, Shared, AgentResource/Body, ViewValue, and nominal forms. | GitHub connector full enum range |
| `crates/arcweft-core/src/entry/identity.rs` | `1-160` | `dabe45e0cf5ddae15ddcc741e83d6b1a8ee0bfcf` | existing RuntimeValueDigest | Existing digest macro owns RuntimeValueDigest and provides ZERO; this design reuses the type but forbids interpreting ZERO as empty arguments. | GitHub connector |
| `crates/arcweft-core/src/value.rs` | `430-520` | `6370e9bee5594bf3ec5835b5ab00405570a27a8f` | current RuntimeValue | Current closed RuntimeValue variants include String/EntityRef/Tuple/etc. and no NeedHandle; the original enum must gain the new variant and inherent behavior. | GitHub connector |
| `crates/arcweft-core/src/value.rs` | `1000-1030` | `6370e9bee5594bf3ec5835b5ab00405570a27a8f` | current canonical value API | RuntimeValue::try_digest currently obtains canonical bytes then hashes, proving the intermediate allocation replaced by one sink-parametric visitor. | GitHub connector |
| `crates/arcweft-core/src/entry/schema.rs` | `1-650` | `24e5bbc3cd3409f3f06ea08a81108574babb4626` | canonical RuntimeValue byte grammar | One exhaustive CanonicalRuntimeValueBytes encoder owns stable value tags/order; tag 20 is available for NeedHandle and current Range/Iterator/Function lack replay/save encoding. | GitHub connector, including canonical writer lines 360-650 |
| `crates/arcweft-runtime-plan/src/semantic_facts.rs` | `1-360` | `6e984a9a0c166c134d8e724b99261ac380852bf2` | runtime normalized type projection | Current normalized type owner already carries exact opaque producer/admission/value_class/persistence in runtime projection and preserves accepted semantic type identity. | GitHub connector |
| `docs/02-runtime/async-scheduler.md` | `1-260` | `e070aa843839be76654df6c61b2ccf4c237d249e` | maintained scheduler/Need contract | Maintains JoinSameKey task boundary, Sans-I/O TaskHost, Need<Result> domain-failure rule, Await/AwaitMany behavior, and timeout ordering. | GitHub connector |
| `docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/CHECKED_MATCH_COVERAGE.md` | `1-160` | `4f73f785b229d7e3d34785ab472b94970dcd880a` | retained predecessor coverage design | Provides bounded typed Maranget pattern matrix and exact limits retained here, subject to the request's corrected literal-only guard authority. | GitHub connector frozen mirror |
| `docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/README.md` | `1-120` | `ef1b0b26aab7ab57d2247406bf7fda9011297441` | predecessor frozen mirror index | Records predecessor claimed readiness, SHA/request identity, reading order, and prior owner selections superseded where intake/current request identify contradictions. | GitHub connector frozen mirror |

## Input-copy evidence

| File | Bytes | SHA-256 |
|---|---:|---|
| `inputs/CURRENT_REQUEST.md` | 26729 | `0152f1dd5f6fd315722f729700d3b94d1b0daa596a59445313e7796bddde8322` |
| `inputs/RUST_SKILL.txt` | 5045 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| `inputs/PROJECT_PREMISE.txt` | 250 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` |

`inputs/CURRENT_REQUEST.md` is byte-equal to the current repository request
blob `7ed008dec6eddb820e228ea0803bf97a1ead2c36`.

## Predecessor binary evidence boundary

The predecessor ZIP is retained in the repository with expected SHA-256
`DDD097E8057A8D45018528431790C20A2DE665CDE40F0329B82CB0366CF95D32`. The current repository intake reports that archive
safety/integrity and its internal validator passed. The connector response for
the binary was truncated, so this return did not independently stream and
rehash the complete predecessor ZIP. Its frozen textual mirror and intake were
inspected. No stronger binary verification is claimed.

## Evidence interpretation

The rows above support the selected design direction. They do not claim that
production already contains the final schemas. `TEST_MATRIX.md` distinguishes
specified implementation gates from commands actually run for this archive.
