# Repository evidence: implemented substrate versus proposed correction

## Inspection identity

- Repository: `Sanzentyo/arcweft` (private; inspected through the configured GitHub connector)
- Branch/ref: `main`
- Exact inspected commit: `23ed5d93824630d8ead9092d32f7fc70f0a8f314`
- Inspection date: 2026-07-21 (Asia/Tokyo)
- Production implementation performed: **none**

## Evidence ledger

| Area | Path/ref | Blob/ref identity | Observed fact | Classification |
| --- | --- | --- | --- | --- |
| Repository head | main | 23ed5d93824630d8ead9092d32f7fc70f0a8f314 | Branch head compared equal to inspected revision; all rows below use this ref | Verified static metadata |
| Repository instructions | AGENTS.md | ea4a46132ff8cd004f860c89c854e4cbfe807d86 | Read through line 447; package/ownership, no source gate, no compatibility, Sans-I/O, compile-clean/audit rules | Implemented substrate/rules |
| Lang-01.1.1 request | docs/reviews/requests/2026-07-17-lang-01.1.1-direct-style-suspension-generator-contract-correction.md | 43af13… | Ordinary fn/direct suspension/generator substrate is authoritative; provisional Stream wire is not | Accepted design input named by sole request |
| Lang-01.3.1.1 request | docs/reviews/requests/2026-07-18-lang-01.3.1.1-external-stream-callable-surface-reconciliation.md | 69ed30… | External Stream is an ordinary bodyless callable using shared resolver/catalog | Accepted design input named by sole request |
| Lang-01.3.1.2 request | docs/reviews/requests/2026-07-18-lang-01.3.1.2-typed-stream-runtime-wire-model.md | 347b0c507f980b360c0326354ebfec1bcce87315 | Selected typed identity/affine/Source deletion direction but left correction defects | Superseded fields identified |
| Typed await implementation evidence | docs/implementation/2026-07-20-lang-01.1.1-await-source-slice.md | inspected at main | Typed AwaitExpr/ranges landed; no source reconstruction; no final generator Stream wire yet | Implemented substrate |
| Shared callable schema | crates/arcweft-lang-sema/src/callable/schema.rs | cd071c7f987c9a6a56ae62e1c0617fe4e4d2381d | Five passing modes, three presence modes, exact CallableParameterSource spans already exist | Implemented/retain |
| Shared callable resolver | crates/arcweft-lang-sema/src/callable/resolver.rs | 01502c0795e847bbf58a4d0ea5e44b804960e164 | Existing checked resolver/query budget is the sole binder | Implemented/retain |
| Semantic effect inventory | crates/arcweft-lang-sema/src/effects.rs + effect_row.rs | 5e20ee… / 5e7a… | Typed EffectId/BTreeSet inventory exists; RuntimePlan table owner is missing | Reuse + proposed projection |
| Current Stream core | crates/arcweft-core/src/stream.rs | 90edb7389d24fb8524908a911738c8cd7d37e5ef | Imports SourceEventKind; unchecked emitted_count increment; close clears queue | Concrete defect |
| Current Source core | crates/arcweft-core/src/source.rs | 8dc8478ad0d1d61a5f26b297ce5f570413fd0129 | SourcePlan/state/policy/event family owns duplicate runtime path; default LatestOnly/EventOnly/Transient/capacity1 | Delete path; default preserved semantically |
| Current RuntimePlan owner | crates/arcweft-core/src/plan.rs | 8e3f62555ed88903950d1ed68871c0f64855b7eb | Existing entry/callable/flow inventories and dependency owner established | Retain/revise narrowly |
| Current accepted manifest owner | crates/arcweft-launch/src/manifest.rs + accepted.rs + source_map.rs | e86be861… / 6768e746… / 0a9d613… | Private `ProfileSpec` is selected from one `SourceBackedManifest`; exact fields use revision-bound `SourceSpan` source-map entries; no Stream profile field exists yet | Reuse/extend existing owner |
| Current sibling dependency boundary | crates/arcweft-compiler/Cargo.toml; arcweft-runtime-plan/Cargo.toml; arcweft-launch/Cargo.toml | d3de0fa… / f8a1059… / f75787f… | Compiler already depends on launch and runtime-plan; runtime-plan does not depend on launch and launch does not depend on core/runtime-plan | Preserve through compiler-owned projection |
| Current RuntimeStep | crates/arcweft-core/src/step.rs | 92a34b177b7474ccaafaaa8922b6d332fb535c95 | Source ingress, separate Source/Stream egress, source_close, usize statistics | Concrete boundary defect/change target |
| Current FiberState | crates/arcweft-core/src/awbc/fiber.rs | 5f46f3fc91fce24b0a9b58b8fa26c62b15dd0570 | Executor-neutral exchange exists but imports Source/old Stream plan state | Retain exchange/remove duplicate instance owners |
| Current product snapshot | crates/arcweft-core/src/awbc/product_step/snapshot.rs | f788e364cf079837e5ba42f39845cba2c5727d21 | stream_sequences plus compact→facade rebuild functions create duplicated state representations | Concrete sole-owner defect |
| Current AWBC schema | crates/arcweft-core/src/awbc/schema.rs | 01c0d41efb396db7292b9104b30a035441ca4372 | ABI1; old Stream/Source tables, opcodes/function kinds; next tag ranges confirmed | Allocation baseline |
| Current AWBC code codec | crates/arcweft-core/src/awbc/codec/code.rs | 531b5f40be683a1ba3049e27887c591987e6a665 | Function-kind and opcode tags are explicit, strict, single-reader | Allocation/rejection baseline |
| Current AWBC wire primitives | crates/arcweft-core/src/awbc/codec/wire.rs | bbdf6f2dc3c624b23f78f8d92730a64d76946440 | u8 tags, u32 canonical varints, u64 LE, length/budget/trailing checks | Retain codec substrate |
| Current AWBC verifier | crates/arcweft-core/src/awbc/verify/structure.rs | 1922462e4246290eab6de05dbadb1846612a4bc4 | Exact ABI check, canonical strings/effect members, table/reference validation | Retain/extend in owner |
| Current digest identities | crates/arcweft-core/src/entry/identity.rs | 4a4c982978cb3079f984b5f7bc0ca05fcd407bef | Existing TypeLayoutHash/contract digest owner confirmed | Reuse, no Stream-local copy |
| Current RuntimePayload | crates/arcweft-core/src/value.rs | 25ee59e63f9354d357d283f067ab1123804b0d89 | Typed RuntimeValue payload shape exists and is not debug string | Retain shared payload codec |
| Current bundle schema | crates/arcweft-bundle/src/lib.rs | 568c887d0e7170d30ed686c107f669c1980cfec6 | Schema5, AwbcV1, runtime summary has stream_plans/source_plans | Version-cut target |
| Current bundle AWBC wrapper | crates/arcweft-bundle/src/product_awbc.rs | 4d284… | Single canonical AWBC product executable wrapper and verifier integration | Retain pattern/change version atomically |
| Current save schema | crates/arcweft-runtime-driver/src/session_save.rs | c5fd9c392092d47a29fee26c3ea9545dce95cb04 | BUNDLE_SESSION_SAVE_SCHEMA_VERSION=1 and current executor snapshot owner | Schema2 target |
| Current strict save decoder | crates/arcweft-save/src/lib.rs | e503c… | Strict schema/version decoder with optional registered migration pattern | Use exact schema2 and register no schema1 migration |

## Implemented substrate retained

The package treats the shared callable catalog/resolver and source spans, typed await and
direct-call/CFG/frame substrate, executor-neutral FiberState exchange, strict AWBC/save
single-version patterns, executable identity/debug-map exclusion, RuntimePayload, and
atomic transaction boundaries as implemented/verified substrate. It prescribes narrow
owner changes only where the sole request identifies a contradiction or current code
shows the concrete duplicate/queue/counter/wire defect.

## Proposed production changes

Everything described as `RuntimeStreamDefinition`, `StreamInstanceTable`, replay store,
corrected policy profile, RuntimeStep Stream boundary, codec8 tags/tables, bundle6,
save2, and Source deletion is a contract proposal for subsequent implementation. This
archive contains no modified repository files and makes no claim that those changes
compile or pass runtime tests yet.

## Static-inspection boundary

Repository evidence was read from the exact private-repository ref through connector file
and search APIs. A local source checkout was not available in the artifact container, so
no Cargo command or structure audit was executed against main while producing this
contract. `VERIFICATION_REPORT.md` separates those deferred implementation gates from the
package-integrity checks actually executed.
