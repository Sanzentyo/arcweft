# Compile-clean implementation order

## P01 — public checked cross-crate primitive vocabulary

**Add:** core plan/AWBC typed IDs, declarations, wrappers, slot/site enums, checked constructors; trybuild pass/fail harness

**Migrate:** none; new types compile beside current raw structs

**Delete in the same phase:** any parent-prototype pub(crate) constructors or duplicate retry enums if present; do not add caller gates

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-runtime-plan && cargo test -p arcweft-core --test ui && cargo test -p arcweft-runtime-plan --test ui`

**Exit:** real external lowerer test compiles primitive construction; unrelated literals/IDs/wire DTOs fail

## P02 — complete plan-node algebra and exhaustive RuntimeExpr/Pattern wrappers

**Add:** RuntimeOperationalType, RuntimePlanTypeKind, RuntimeExpr::try_visit_nodes, exact typed expr/pattern constructors

**Migrate:** arcweft-runtime-plan final_expr/final_pattern/final_flow emit root [0] and exact children/bindings through public APIs

**Delete in the same phase:** non-root/non-node success branch; ad hoc recursion helpers/extension traits; conceptual untyped rows

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-runtime-plan && cargo test -p arcweft-runtime-plan final_expr final_pattern`

**Exit:** all legal checked/operational expression families lower with exact complete node sets

## P03 — final RuntimePlan builder and private raw aggregate

**Add:** RuntimePlanBuilder, RuntimePlanWireV1 private DTO, custom Serialize/Deserialize, accessors

**Migrate:** runtime-plan, compiler, verifier, bundle/tests construct/read through builder/accessors

**Delete in the same phase:** RuntimePlan public fields, Default, derived Deserialize, unchecked direct table mutation, alternate wire reader

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-verify -p arcweft-bundle`

**Exit:** no workspace consumer needs RuntimePlan struct literal or mutable fields

## P04 — final AwbcProgram builder and private raw aggregate

**Add:** AwbcProgramBuilder, typed raw primitives/origins, private AwbcProgramWireV1, accessors

**Migrate:** runtime-plan awbc_lower and all codec/verifier/compiler/bundle tests use checked builder/accessors

**Delete in the same phase:** AwbcProgram public fields, Default, derived Deserialize, unchecked origins/typed DTOs, alternate reader

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-verify -p arcweft-bundle`

**Exit:** real AWBC lowerer pass case builds every current table without core-private access

## P05 — core-only generation projection and immutable issuer

**Add:** projection row types/builder, catalog projection, canonical AWGF fact-section encoder/decoder, transcript, AdmittedRuntimeGeneration::try_issue/fact_section and public require_same_parent comparison

**Migrate:** generation scalar/newtype consumers to declaration accessors; compiler/bundle persistence reads the issuance-created fact section; add generation admission/fact-wire unit/property tests

**Delete in the same phase:** any upward runtime-plan fact stored by core; public/free generation declaration constructor; projection From raw artifacts

**Compile/test scope:** `cargo check -p arcweft-core && cargo test -p arcweft-core generation_admission`

**Exit:** core stores only core rows plus exact issuance-created fact bytes; projection/order/root/duplicate/transcript/AWGF round-trip tests pass; downstream crates compare exact parents without constructing one

## P06 — accepted-world compiler assembly and issue-before-lowering orchestration

**Add:** AcceptedRuntimeOpaqueProducerRegistry; ProjectRuntimeGenerationAssembly; accepted owner projections

**Migrate:** compiler project flow issues generation before invoking runtime-plan; lowerers receive immutable declaration clone

**Delete in the same phase:** free generation scalar injection, raw declaration-to-fact conversion, any plan/AWBC-derived fact branch

**Compile/test scope:** `cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler && cargo test -p arcweft-compiler generation`

**Exit:** accepted world projects once; tampered raw artifacts cannot alter generation

## P07 — plan admission against issued generation

**Add:** AdmittedRuntimeGeneration::try_admit_plan, AdmittedRuntimePlan, resolved checked/operational rows, site traversal

**Migrate:** compiler/verifier/AOT plan consumers that require semantic authority to admitted plan

**Delete in the same phase:** RuntimePlan::try_admit/self-admission prototype, optional root authority, root-use rows, unchecked operational boundary success

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-compiler -p arcweft-verify -p arcweft-aot && cargo test -p arcweft-core plan_admission`

**Exit:** plan declaration/site completeness and operational-root tests pass against independent generation

## P08 — context/domain issuance and atomic opaque semantics

**Add:** admitted-plan/product context methods, private RuntimeCheckedValueContext, exact atomic opaque errors/work rules

**Migrate:** nominal record construction, checked constants/host inputs, diagnostics to issued contexts; physical ownership path remains separate

**Delete in the same phase:** pre-plan checked_values context, payload lookup/recursive opaque branch, checked OpaquePayload step, placeholder domain parent

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-dialogue && cargo test -p arcweft-core checked_value opaque nominal_record`

**Exit:** opaque atomic matrix and non-circular context tests pass

## P09 — effect-owned AudioCommand sites

**Add:** AudioCommand { effect, slot }, inherent AwbcAudioCommand typed_values/typed_value, effect-based resolver/tags

**Migrate:** runtime-plan lowerer, AWBC verifier/codec/tests and origin tables to effect coordinates

**Delete in the same phase:** command-only audio coordinate, AwbcEffectPlanTypedSlot::AudioValue, generic numeric audio slot and exclusion tests

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-runtime-plan && cargo test -p arcweft-core awbc_audio && cargo test -p arcweft-runtime-plan awbc_lower`

**Exit:** all 35 fields, optional bus, reused-command aliases, bounds and cycle cases pass

## P10 — AWBC admission and direct same-parent pair

**Add:** generation.try_admit_awbc, AdmittedAwbcProduct, admitted_plan.try_admit_awbc, AdmittedRuntimeProduct

**Migrate:** compiler/verifier/bundle/AOT outputs and pair tests to admitted wrappers/direct origins

**Delete in the same phase:** AwbcProgram::try_admit/self-admission prototype, root-use maps, optional authority, operational origin success, second correlation digest

**Compile/test scope:** `cargo check -p arcweft-core -p arcweft-compiler -p arcweft-verify -p arcweft-bundle -p arcweft-aot && cargo test -p arcweft-core runtime_product`

**Exit:** same-parent, declaration tamper, direct equality and operational-origin tests pass

## P11 — catalog, runtime-driver, VM/executor, and hot-swap migration

**Add:** AdmittedCharacterDialogueCatalogs, RuntimeDriverGeneration, admitted AwbcProductStepExecutor, prepared swap holding exact current admitted parent

**Migrate:** dialogue/View/character runtime, runtime-driver generation runtime, VM/fiber/product-step and live swap consumers

**Delete in the same phase:** raw executor constructors/program accessor/raw replacement, free catalog generation scalar, non-atomic cross-generation swap

**Compile/test scope:** `cargo check -p arcweft-dialogue -p arcweft-view -p arcweft-character -p arcweft-runtime-driver -p arcweft-core && cargo test -p arcweft-runtime-driver`

**Exit:** same-parent driver/catalog, rollback, executor, exact-parent ABA, and hot-swap tests pass

## P12 — bundle/AOT/snapshot/restore/replay generation-first cut

**Add:** BundleSectionKind::RuntimeGenerationFacts=23 and ::RuntimePlan=24 on the existing enum, admitted-product section writer, owned VerifiedRuntimeGenerationSections token, checked loader, fixed generation headers, product-issued value decode contexts

**Migrate:** existing BundleSectionKind inherent mapping/policies and REQUIRED_PROGRAM_SECTIONS; bundle reader/writer, compiler object output, AOT, save/snapshot, restore/replay and cache consumers

**Delete in the same phase:** public raw section load fields/accessors, unrelated raw triple writer, plan/AWBC-first decode, raw artifact generation issuance, string-only value path, dual reader, decode-before-context branch

**Compile/test scope:** `cargo check -p arcweft-bundle -p arcweft-aot -p arcweft-save -p arcweft-runtime-driver -p arcweft-compiler && cargo test -p arcweft-bundle runtime_generation && cargo test -p arcweft-runtime-driver restore replay`

**Exit:** unique schema-v1 section tags 23/24/1 verify before decode; generation mismatch fails before executable/value decode; all format markers remain 1

## P13 — complete tests, generated fixtures, schemas, stable docs, and inventory synchronization

**Add:** all TEST_MATRIX rows, mapping tables/goldens, maintained schema/design docs and generated fixtures

**Migrate:** stale parent tests/docs to selected final authority; regenerate deterministic artifacts

**Delete in the same phase:** contradictory audio/Option/opaque/expression tests, source gates, stale aliases and outdated examples

**Compile/test scope:** `cargo test --workspace --all-features; focused generated-artifact comparisons; no source-spelling acceptance scans`

**Exit:** normative matrices and production behavior agree; every affected consumer has an inventory row

## P14 — workspace closure and physical deletion audit

**Add:** none except final evidence records

**Migrate:** none; fix only compile/test/audit defects exposed by final owners

**Delete in the same phase:** all obsolete prototypes, dead variants/helpers/readers/fallbacks/temporary public fields and compatibility remnants

**Compile/test scope:** `cargo fmt --all -- --check; cargo check --workspace --all-targets --all-features; cargo clippy --workspace --all-targets --all-features; cargo test --workspace --all-features; cargo +nightly -Zscript tools/structure-audit.rs --root .`

**Exit:** clean Git diff limited to implementation cut; all selected gates pass; no version other than 1

No phase permits a placeholder generation, self-admission fallback, temporary public field, validation-off success branch, compatibility wrapper, dual reader, or version increment.
