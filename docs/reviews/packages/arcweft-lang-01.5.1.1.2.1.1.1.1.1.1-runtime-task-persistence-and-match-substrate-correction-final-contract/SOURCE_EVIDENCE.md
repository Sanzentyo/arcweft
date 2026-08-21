# Current source evidence

Repository: `Sanzentyo/arcweft`  
Inspected `main`: `3670625a02b9e7e8578b57fc7b148a1758a17dba`  
Request-stated production: `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`  
Head delta: one documentation-only audit commit; production crates are unchanged.

| Path | Blob/evidence | Current owner observation | Verification |
|---|---|---|---|
| `AGENTS.md` | `connector-inspected at current SHA` | one typed authority, deletion-driven migrations, design-only package boundaries, and compile-clean cut expectations | authenticated GitHub connector; complete file read |
| `docs/AGENTS.md` | `connector-inspected at current SHA` | documentation/package ownership and review artifact requirements | authenticated GitHub connector; complete file read |
| `docs/reviews/AGENTS.md` | `connector-inspected at current SHA` | independently throwable final-contract ZIP, no generic CLOSED placeholders, validator and source evidence requirements | authenticated GitHub connector; complete file read |
| `docs/implementation/AGENTS.md` | `connector-inspected at current SHA` | implementation reports must distinguish verified source evidence from specified future production work and must not treat absent paths as targets | authenticated GitHub connector; complete file read |
| `crates/AGENTS.md` | `connector-inspected at current SHA` | original enum owners receive inherent behavior; no ad-hoc extension traits or unsafe shortcuts | authenticated GitHub connector; complete file read |
| `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction.md` | `6b3d614e7813fa6552e84f15610175633470227d` | ten mandatory corrections, 14 package categories, negative validator crossings and READY gate | repository file and 415-line attached input read completely; attached SHA-256 804f68c052640fe3964e70bfe011cad2c4429873a70b790c3a0526b5f46c7e6e |
| `docs/implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-1-runtime-need-view-return-intake.md` | `connector-inspected at current SHA` | nine repository crossings that invalidate the predecessor return despite internal package consistency | authenticated GitHub connector; complete file read |
| `docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract/IDENTITY_AND_DIGESTS.md` | `connector-inspected at current SHA` | producer/Need/task ID transcripts, policy ordinals, family set, RuntimeValue tag 20 and View identity exclusions | authenticated GitHub connector; retained sections inspected; frozen without renumbering |
| `crates/arcweft-core/src/task.rs` | `130256a8a8efb2fe6c7028c68357cb707f975eb9` | String-backed NeedId/TaskKey/TaskId, unconditional TaskSpec.request and incomplete correlation/events | authenticated GitHub connector at inspected SHA; production unchanged by docs-only head commit |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | `connector-inspected at current SHA` | separate lightweight pending/in-flight scheduler with no generation journal, adapter transaction, runtime task state or snapshot owner | authenticated GitHub connector; relevant file read |
| `crates/arcweft-runtime-driver/src/task.rs` | `22aabea946be7645afcebe65415bcea3cd786eb9` | second RuntimeTaskRegistry and partial String/Option DTO authority | authenticated GitHub connector; complete relevant range |
| `crates/arcweft-runtime-driver/src/swap.rs` | `39ecdfad33abfe4a55e20f8e7501878152ef797e` | driver-local GenerationId proves the required move to arcweft-core | authenticated GitHub connector |
| `crates/arcweft-core/src/value.rs` | `6370e9bee5594bf3ec5835b5ab00405570a27a8f` | closed enum lacks NeedHandle and derives structural PartialEq; contains_nonconstant_opaque couples constant policy | authenticated GitHub connector; relevant ranges inspected |
| `crates/arcweft-core/src/value/opaque.rs` | `connector-inspected at current SHA` | RuntimeOpaqueValueClass Plain/AffineHandle and RuntimeOpaquePersistence ConstantAndSnapshot/SnapshotOnly are current exact owners | authenticated GitHub connector; complete file read |
| `crates/arcweft-core/src/entry/schema.rs` | `24e5bbc3cd3409f3f06ea08a81108574babb4626` | one exhaustive canonical visitor currently rejects SnapshotOnly opaque identity and materializes bytes before hashing | authenticated GitHub connector; canonical writer inspected |
| `crates/arcweft-core/src/value/awbc_save.rs` | `connector-inspected at current SHA` | complete AWBC-specific value snapshot shape provides the concrete base to generalize to core RuntimeValueSnapshotV1 | authenticated GitHub connector; complete relevant range |
| `crates/arcweft-core/src/value/agent.rs` | `61ef48b8b74c80120bf3f6777c708597b851bff5` | exact RuntimeAgentValue variants include ActionTarget, CaptureTarget, DebugStatePath, ObservationFieldPath, Probe, Diagnostics, Predicate and ViewportPoint | authenticated GitHub connector; enum and payloads inspected |
| `crates/arcweft-core/src/plan/type_kind.rs` | `connector-inspected at current SHA` | closed RuntimePlanTypeProjection and RuntimeAgentTypeProjection; Predicate is a leaf while Probe alone carries a child | authenticated GitHub connector; exhaustive children() inspected |
| `crates/arcweft-lang-sema/src/types.rs` | `528d32482865aae4bf8dd8f456468b061ecdedc7` | 85 exact current variants used to regenerate the carrier-backed matrix | authenticated GitHub connector; full enum range inspected |
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | `a415ac5e8ae90533a6983e1c934185bd15395848` | 27 CheckedExpressionResolution variants, 8 CheckedValueResolution variants, 7 CheckedSelectResolution variants and 5 CheckedPatternResolution variants | authenticated GitHub connector; exhaustive definitions inspected |
| `crates/arcweft-lang-sema/src/final_analysis/report.rs` | `31a587463d8f035b4d753c0fcbc7fcfcf75d3876` | FinalSemanticAnalysis owns exact per-module HirSnapshotId and expression/pattern maps; no AcceptedSemanticGeneration owner exists | authenticated GitHub connector; report construction and generation validation inspected |
| `crates/arcweft-lang-hir/src/pattern.rs` | `e35576238711d9a331d267fbfac4af52d32eea7f` | 13 exact HirPatternKind variants including Error recovery | authenticated GitHub connector; full enum inspected |
| `crates/arcweft-lang-hir/src/leaf.rs` | `6011efa3dbaa20b5964e3b83b9b1eabf8c20df42` | 7 exact literal families and canonical big integer, decimal, float-bit and duration semantic values | authenticated GitHub connector; literal definitions inspected |
| `crates/arcweft-need/src/lib.rs` | `05446a144a25aaf55e65cd1319d8fe5ad1123f03` | Need<T> is NotStarted/Pending/Ready/Cancelled and Progress validates finite 0..=1 | authenticated GitHub connector; complete file read |
| `docs/02-runtime/need-timeout.md` | `connector-inspected at current SHA` | RuntimeStepInput.dt-only clock, first-demand start, cancellation/source/expiration/Pending precedence and source non-cancellation | authenticated GitHub connector; complete document read |
| `docs/02-runtime/async-scheduler.md` | `e070aa843839be76654df6c61b2ccf4c237d249e` | Sans-I/O TaskHost, JoinSameKey, AwaitMany source-order outputs and domain/infrastructure/cancellation separation | authenticated GitHub connector; complete document read |
| `crates/arcweft-view/src/view/identity.rs` | `153fa4450cc676e7f6b860c9700dfc7bb95e4315` | ViewProgramId and AcceptedViewProgramRevision([u8;32]) are the existing final roles | authenticated GitHub connector |
| `crates/arcweft-compiler/src/view.rs` | `6bba13ab04869dddf95dc6198522820f5d09da99` | real compiler View product path for compiler-local row migration | authenticated GitHub connector; path and owner confirmed |
| `crates/arcweft-bundle/src/product.rs` | `e110bfdb7245fb475827c003e261dfe221e0e768` | real product owner for persistent digest projection | authenticated GitHub connector; path confirmed |
| `crates/arcweft-bundle/src/container.rs` | `c9a52c99c4ac4c59c402f31b62038111e84682ff` | real strict product/container codec owner; no nonexistent codec.rs target is named | authenticated GitHub connector; directory listing confirmed |

## Repository reconciliation conclusions

1. The current canonical RuntimeValue grammar is centralized enough to support one sink-parametric visitor; no new digest owner is needed.
2. The current TaskSpec and driver/scheduler split cannot implement runtime-owned Timeout/AwaitMany or atomic ordinal/adapter publication; the scheduler must become the sole owner.
3. The current `FinalSemanticAnalysis` and `HirSnapshotId` are sufficient to construct exact compiler-local Match references; a new semantic generation type would duplicate authority.
4. The current RuntimeValue/AWBC save/Agent/type projection owners provide concrete evidence for the corrected ownership matrix, while Shared has no four-owner carrier set.
5. Real adapter targets are `arcweft-host-adapter`, `arcweft-adapter-desktop`, `arcweft-player-web`, `arcweft-runtime-host` and `arcweft-agent-runner`; no nonexistent native/web/headless adapter crates are named.

## Verification boundary

- The attached request, Rust skill and project premise are copied byte-for-byte into `inputs/`.
- Attached request SHA-256: `804f68c052640fe3964e70bfe011cad2c4429873a70b790c3a0526b5f46c7e6e`.
- Rust skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`.
- Project premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`.
- Source inspection used the authenticated GitHub connector at the complete current SHA.
- No production checkout/patch/build was performed because this return is design-only.
- Parent ZIP expected SHA-256 `2B9B55043E8168D99838C81048E13F752A75B03F48293010BB36B5401043DB0B` is retained from the request. The frozen mirror and intake were inspected; the predecessor binary was not independently downloaded/rehashed.
- This package itself is fully hashed and validated, including negative self-tests.
