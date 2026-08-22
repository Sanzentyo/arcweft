# Source and evidence inventory

## 1. Repository snapshot

- Repository: `Sanzentyo/arcweft` (private)
- Inspected ref: `main`
- Full Git SHA used: `515bb071437c3af053f1560c3119906dc8002efc`
- Inspection method: GitHub connector reads pinned to the full SHA
- Local clone/working tree: not used; cleanliness and untracked files therefore
  were not observable and are not claimed
- Production files modified by this return: none

## 2. Instructions read in full

| Path/input | Observation used |
|---|---|
| attached `Rust Skill.txt` | Rust API privacy, newtypes, owner APIs, no unsafe/unstable, fmt/Clippy implementation obligations |
| attached project premise | inspect current Arcweft理念/structure before design |
| repository `AGENTS.md` | latest-main precedence; one typed authority; deletion-driven migration; no helper/extension duplication; all versions one |
| repository `crates/AGENTS.md` | layer direction, Sans-I/O core, deliberate API, structured Cargo graph, structural audit policy |
| repository `docs/AGENTS.md` | maintained-doc precedence and evidence requirements |
| repository `docs/reviews/AGENTS.md` | full request/parents/predecessors; design-only archive requirements; READY/open-question rules |

## 3. Current production source inspected

| Path at inspected SHA | Current evidence and design consequence |
|---|---|
| `crates/arcweft-core/src/plan.rs` | current immutable `RuntimePlan` owns type/local/nominal/variant/function/dialogue/entry/callable/flow/helper/trait/line/stream tables and no task-plan table; final table belongs here |
| `crates/arcweft-core/src/plan/construction.rs` | `RuntimePlanBuilder` is the sole mutable aggregate; finish materializes a private complete plan, verifies, then returns; common sealer is inserted before public construction |
| `crates/arcweft-core/src/task.rs` | current production still has legacy String-backed task IDs/templates at this snapshot; this child remains design-only and relies on the accepted Cut 5 parent switch |
| `crates/arcweft-core/src/awbc/schema.rs` and codec/VM consumers found by repository search | AWBC already owns task-plan products and remains owner tag 1; no parallel structured redesign |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | current `ValidatedViewProgramResource` privately owns actual `ViewProgramId`, accepted revision, source-set revision and performs complete validation before publication; evolve this owner with bindings |
| `crates/arcweft-view/src/view/identity.rs` | actual `ViewProgramId(PublicId)` and `AcceptedViewProgramRevision([u8;32])`; revision is current semantic content stamp, not copied into core |
| `crates/arcweft-core/Cargo.toml` | no `arcweft-view`/bundle dependency; preserve this |
| `crates/arcweft-bundle/Cargo.toml` | direct dependencies on both core and View; legitimate upper join layer |
| `crates/arcweft-runtime-plan/Cargo.toml` | depends on core/HIR/syntax, not View; lowering should construct marker rows/coordinates, not actual View projections |
| `docs/00-overview/crate-map.md` | core is runtime/data core, runtime-plan lowers to core, bundle owns codecs/resources, View remains separate |

## 4. Current/accepted review evidence inspected

| Path | Retained decision/evidence |
|---|---|
| attached current request | exact child decisions, transcript, fixed dependency, tests, archive shape |
| `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction.md` | Cut 1–5 publication discipline and retained task/Need/View substrate |
| `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction.md` | accepted handle/batch/snapshot boundaries and Cut 5 atomicity |
| `docs/reviews/packages/...runtime-need-instance-view-match-admission.../IDENTITY_AND_DIGESTS.md` | authoritative task-plan owner/family/class/binding tags and explicit exclusions |
| same package `RUST_SCHEMAS.md` | retained typed Need/task identity roles; superseded raw construction where current child narrows it |
| `docs/reviews/packages/...runtime-task-persistence-and-match-substrate.../VIEW_BUNDLE_PROJECTION.md` | compiler-local Cut 3 row and stable View site; raw projection model superseded by current child actual upper owner |
| `docs/reviews/designs/...runtime-launch-receipt-keyed-ordinal-and-current-owner/FINAL_DESIGN.md` | one core task-validation authority and one View protocol; core does not import View/bundle |
| same design `schemas/final_contract.rs` | previous provisional self-digest RuntimeTaskPlan is evidence of what current request deletes, not final schema |
| `docs/implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-checked-match-need-identity-return-intake.md` | prior cyclic/nonconstructible View/self-digest failures and selected correction direction |

## 5. Request-to-source reconciliation

1. Current core plan has the correct sole mutable/final owner boundary but no
   final task-plan table. This contract adds the table only after seal.
2. Current bundle validated View resource is the only inspected production type
   already capable of naming both actual View identities and core resources.
3. Current core manifest proves the no-View dependency is real and enforceable.
4. Prior package prose that used raw View projections or a self digest is
   superseded by the attached child request and is marked for deletion.
5. The accepted `TaskPlanSemanticDigest` transcript and family/class/binding
   tags are retained verbatim.

## 6. Verification scope of this return

Actually verified here:

- full attached request/Rust skill/premise read;
- current private repository source/docs at the stated full SHA inspected;
- package validator and all included negative self-tests executed;
- manifest, hashes, member paths, duplicate/case-fold safety, and ZIP readback;
- exact returned filename and one top-level wrapper.

Not performed or claimed:

- no production checkout or working-tree cleanliness check;
- no production Rust compilation, focused tests, full tests, Clippy, fmt, AOT,
  generated production artifacts, or platform runtime test;
- no production source/schema/fixture modification.

Those implementation validations are explicitly required by `TEST_MATRIX.md`.
