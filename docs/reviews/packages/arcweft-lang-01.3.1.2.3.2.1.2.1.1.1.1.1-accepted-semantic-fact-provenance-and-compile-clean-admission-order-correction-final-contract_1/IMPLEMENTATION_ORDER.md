# Compile-clean implementation order

Each phase is a coherent final-owner cut. No phase depends on a type introduced later. Every command below names a package present in the current workspace.

## P00 — characterize current main

**Owners:** `all affected crates`

**Add/migrate:** record current tests/source/constructors; no code change.

**Delete in the same phase:** none.

**Focused gate:** `cargo check --workspace --all-targets --all-features`

## P01 — core atomic plan construction owner

**Owners:** `arcweft-core`

**Add/migrate:** add inherent intern_batch, RuntimeIndexPath/typed wrappers/visitors, RuntimePlanBuilder; private RuntimePlan and custom v1 decode; migrate all core tests/struct literals in same phase.

**Delete in the same phase:** public RuntimePlan fields, Default, unchecked/custom derive decode.

**Focused gate:** `cargo test -p arcweft-core plan::`

## P02 — HIR synthetic owner vocabulary

**Owners:** `arcweft-lang-hir`

**Add/migrate:** extend existing HirRuntimeSemanticOwnerInventory inherent impl with exact synthetic site enums/iterators.

**Delete in the same phase:** any parallel synthetic walker.

**Focused gate:** `cargo test -p arcweft-lang-hir runtime_semantic_owner`

## P03 — accepted synthetic semantic facts

**Owners:** `arcweft-runtime-plan; arcweft-compiler`

**Add/migrate:** extend same staging/aggregate; compiler projects exact types; add private into_lowering_parts move boundary.

**Delete in the same phase:** fallback/type-name/runtime-value resolver.

**Focused gate:** `cargo test -p arcweft-runtime-plan semantic_facts && cargo test -p arcweft-compiler runtime_semantic`

## P04 — single-builder final lowering

**Owners:** `arcweft-runtime-plan`

**Add/migrate:** migrate FinalExpr/Pattern/Flow and every helper/function/stream/source consumer to one builder and typed wrappers.

**Delete in the same phase:** raw public final lowerer success paths and independent nested lowerers.

**Focused gate:** `cargo test -p arcweft-runtime-plan final_`

## P05 — nominal field projection and core generation facts

**Owners:** `arcweft-core; arcweft-compiler`

**Add/migrate:** add checked field projection on original owner; root/producer/construction fact builders deriving roots.

**Delete in the same phase:** field literals, unchecked row constructors, caller root maps.

**Focused gate:** `cargo test -p arcweft-core nominal_record && cargo test -p arcweft-compiler nominal`

## P06 — canonical generation aggregate and structural issue

**Owners:** `arcweft-core; character/view/dialogue adapters; compiler`

**Add/migrate:** wire owner-provided catalog facts; add explicit trusted-integrator structural issuance and exact parent token.

**Delete in the same phase:** bare digest/root inputs and self-issued roots.

**Focused gate:** `cargo test -p arcweft-core generation && cargo test -p arcweft-compiler generation`

## P07 — plan admission

**Owners:** `arcweft-core`

**Add/migrate:** consume raw RuntimePlan against independent generation; issue private plan key.

**Delete in the same phase:** RuntimePlan::admit/self-admission and raw execution branches.

**Focused gate:** `cargo test -p arcweft-core plan_admission`

## P08 — AWBC domain schema/builder/v1 codec

**Owners:** `arcweft-core`

**Add/migrate:** add one in-place v1 domain table after runtime_types, handle/remap, record operands, private AwbcProgram/custom decode; migrate all core constructors.

**Delete in the same phase:** public program fields, sidecar domain map, old reader.

**Focused gate:** `cargo test -p arcweft-core awbc::codec && cargo test -p arcweft-core awbc::verify`

## P09 — AWBC lowering/admission/product

**Owners:** `arcweft-runtime-plan; arcweft-core`

**Add/migrate:** lower from AdmittedRuntimePlan through builder; admit exact domains/sites; pair exact Arc parent/key.

**Delete in the same phase:** raw-plan AWBC lower, direct mutation/canonicalize success path.

**Focused gate:** `cargo test -p arcweft-runtime-plan awbc && cargo test -p arcweft-core runtime_product`

## P10 — product-only checked context/domain

**Owners:** `arcweft-core`

**Add/migrate:** add inherent product methods after product exists; migrate nominal checked construction and semantic value admission.

**Delete in the same phase:** pre-product context/domain issuance and RuntimeValue shape reconstruction.

**Focused gate:** `cargo test -p arcweft-core checked_value_context`

## P11 — bundle byte verifier/evidence

**Owners:** `arcweft-bundle`

**Add/migrate:** decode fact/plan/AWBC sections, trust policy, independent admissions, pair; expose byte-only verifier.

**Delete in the same phase:** compiler dependency, generic/nonexistent VerifiedBundle, decoded-section publication.

**Focused gate:** `cargo test -p arcweft-bundle runtime_generation`

## P12 — compiler product/evidence and bridge

**Owners:** `arcweft-compiler`

**Add/migrate:** orchestrate exact compile order; issue private CompilerRuntimeEvidence; encode same v1 transcript and call bundle verifier.

**Delete in the same phase:** placeholder inputs, raw product return, bundle-to-compiler dependency.

**Focused gate:** `cargo test -p arcweft-compiler runtime_product`

## P13 — driver publication and direct core VM

**Owners:** `arcweft-runtime-driver; arcweft-core`

**Add/migrate:** publish only VerifiedRuntimeBundleProduct; bind host policy; direct VM accepts published product.

**Delete in the same phase:** raw plan/program/admitted-product execution and host root/digest parameters.

**Focused gate:** `cargo test -p arcweft-runtime-driver publication`

## P14 — published JIT/AOT/accelerator adapters

**Owners:** `arcweft-lang-jit-cranelift; arcweft-runtime-codegen; arcweft-runtime-accelerator`

**Add/migrate:** add driver dependency and accept only &PublishedRuntimeGeneration.

**Delete in the same phase:** raw plan/AWBC/admitted-product backend entry APIs.

**Focused gate:** `cargo test -p arcweft-lang-jit-cranelift && cargo test -p arcweft-runtime-codegen && cargo test -p arcweft-runtime-accelerator`

## P15 — hot swap/save/restore/replay

**Owners:** `arcweft-runtime-driver; arcweft-save`

**Add/migrate:** same-parent prepared swap; verify/publish before lower save decode; driver performs product-context value admission.

**Delete in the same phase:** save-to-driver dependency, pre-verification semantic decode, scalar-only parent equality.

**Focused gate:** `cargo test -p arcweft-runtime-driver swap && cargo test -p arcweft-runtime-driver restore && cargo test -p arcweft-save`

## P16 — deletion audit, docs, full gates

**Owners:** `workspace`

**Add/migrate:** remove all aliases/fallbacks/raw success branches; regenerate fixtures/docs/inventories.

**Delete in the same phase:** all listed deletion targets.

**Focused gate:** `cargo fmt --all -- --check && cargo check --workspace --all-targets --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && cargo +nightly -Zscript tools/structure-audit.rs --root .`

