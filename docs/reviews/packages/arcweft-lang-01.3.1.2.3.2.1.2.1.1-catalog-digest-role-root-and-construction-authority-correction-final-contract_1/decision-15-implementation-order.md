# Decision 15 — compile-clean implementation and deletion order

Each phase is one compile-clean dependency cut. Later owners may not be temporarily bypassed with unchecked constructors, defaulted fields, name recognition, or raw execution.

1. **Lower vocabulary:** retain/move `CharacterDialogueCustomFieldId` and `CharacterDialogueRuntimeRole` in `arcweft-interaction-model`; add `ALL`, `AUTHORED_BASE`, `canonical_tag`, and `is_authored_base` on the original enum. Preserve serde names, family validation, 128-byte limit, and re-exports.
2. **Core checked substrate:** add little-endian canonical scalar helpers in the legitimate core codec owner; add paths, structured checked errors, validator, shared 65,536 work budget, and exact Choice semantics on `RuntimeCheckedType`; migrate authority consumers away from public boolean acceptance.
3. **Raw declarations and errors:** add project-capable `RuntimeProjectRootError`, root-use/site declarations, AWBC runtime type declaration, AWBC domain declaration/ID, and exact private constructors/accessors. Keep versions at 1 and no deserialization defaults.
4. **Character/View canonical digests:** implement Decision 01 and Decision 02 in their current owners; add accepted View program revision to Arcweft descriptors; close local tests before upper admission work.
5. **Semantic role/custom projection:** add six accepted standard opaque nominal rows, `TypeKind::CharacterDialogueRole`, declaration/registry owner, derived Style, and canonical custom-field projection. Delete the six relevant `Named` success rows in the same cut.
6. **Runtime-plan root facts:** land `runtime_roots.rs`, source evidence, lossless semantic-ID root projection, exact project/producer facts, and complete `RuntimePlanTypedSite` use emission. Fail compilation until every current typed boundary has a row.
7. **Generation admission:** consume project and producer roots, build the one parent `AdmittedRuntimeGeneration`, store only internal per-root nominal traversal closures, compare exact authorization sets, and add runtime-driver generation catalog wrappers.
8. **Raw `RuntimePlan` admission:** add mandatory generation contract/root uses, implement exact site resolution/reachability/error precedence, and make execution constructors consume `AdmittedRuntimePlan` or perform full admission internally.
9. **AWBC schema/codec/lowering/admission:** replace runtime type table shape, add typed-root uses and nominal domain table, extend record constants/`MakeRecord`, update LE codec/golden/tamper fixtures, verifier, pair/standalone admission, and exact plan-site equality.
10. **Execution cut:** migrate VM, fiber, product step, AOT/JIT/codegen, accelerator, executor/session/hot-swap/player APIs to admitted wrappers. Remove raw `AwbcProgram` constructors, replacement APIs, `Deref`, and extraction escapes.
11. **Dialogue:** construct CharacterDialogue schema only from same-generation admitted producer/roles/custom/Character/View borrows; route normalize/clear/patch/encode/decode/digest through typed validation and atomic publication.
12. **Restore/replay/View/bundle/save:** validate raw artifacts and runtime values before ownership traversal, activation, domain decoding, replay, or hot swap; preserve structured error paths.
13. **Unchecked-constructor deletion:** delete public `RuntimeNominalRecordValue::new`, `validate_shape`, descriptorless nominal reconstruction, identity/layout-only validators, public arbitrary role constructors, caller-supplied custom digest, and public boolean authority success.
14. **Consumer/test cleanup:** replace stale helpers/fixtures and remove compatibility counters, old aliases/readers, source-name gates, and unused migration types.
15. **Final gates:** focused tests, codec/golden/tamper tests, compile-fail tests, `cargo fmt --all -- --check`, workspace check/test/Clippy, structure audit, dependency audit, and applicable Tier 2 commands. Record exact command results in an implementation evidence note.

No phase may leave raw plan/AWBC execution or generation-blind nominal construction operational while claiming later-phase readiness.
