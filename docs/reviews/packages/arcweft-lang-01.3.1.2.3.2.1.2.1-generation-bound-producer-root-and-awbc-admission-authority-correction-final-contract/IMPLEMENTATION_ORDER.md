# Compile-clean implementation and deletion order

Every gate ends in a compiling workspace state. Production implementation must
not merge an early gate that leaves a new authority beside a still-reachable
unchecked path.

## G0 — evidence and retained artifact verification

1. Checkout commit `50771a19f57f86570837f616a66252be24e77e0c`.
2. Confirm `HEAD`, `main`, and `origin/main` as required by the request.
3. Read root/scoped `AGENTS.md`, docs/test policy, this package, `.1.2`, and
   accepted G1 commit `1648894fbfc38ba623d1b01c6001fbd55b67b10b`.
4. Verify the retained `.1.2` ZIP with `PARENT_ARTIFACTS.sha256`.
5. Inventory all raw RuntimePlan/AwbcProgram execution call sites and preserve
   the inventory in the implementation note.

Exit: clean tree and exact authority evidence recorded.

## G1 — core canonical types and generation declaration

1. Add typed generation/root/digest newtypes in the specified core owner.
2. Correct the nominal catalog declaration by removing producer-only rows.
3. Add project roots, producer payload root sets, CharacterDialogue payload,
   claimed authorization sets, and generation declaration.
4. Add canonical checked-type/custom/generation encoders and limits.
5. Add owning error variants.
6. Unit-test canonical order, limits, identity recomputation, and Serde tamper.

No operational wrapper is exposed yet.

Exit: core focused tests, fmt/check/clippy green.

## G2 — semantic typed roles and custom projection

1. Add `CharacterDialogueRuntimeRole` to interaction-model.
2. Add the original `TypeKind::CharacterDialogueRole` variant and its accepted
   substitution behavior.
3. Add the accepted semantic role owner with six base roles and derived Style.
4. Replace current Named callable-family rows.
5. Extend the existing sema custom-field owner to expose typed projection facts.
6. Add exact source/world/error tests.
7. Prove no Name-based role resolver remains.

Exit: interaction-model/sema focused tests and workspace check green.

## G3 — runtime-plan generation projection

1. Add `RuntimeCharacterDialogueProducerFacts` to the existing semantic-facts
   owner.
2. Project all role/custom checked types through the existing normalized type
   machinery.
3. Retain source evidence and reject leaked Named/role coordinates.
4. Build canonical nominal catalog from the retained Arc interning map.
5. Enumerate typed project roots.
6. Derive producer authorization sets from roots and construct one generation
   declaration.
7. Attach the same declaration to RuntimePlan and AWBC lowering inputs.

Exit: runtime-plan/compiler focused tests and workspace check green.

## G4 — RuntimePlan whole-generation admission

1. Add the required private RuntimePlan field.
2. Implement declaration/catalog/root/custom/identity validation.
3. Implement independent project and producer traversal.
4. Enforce exact claimed authorization and global reachability equality.
5. Add non-Serde admitted aggregate/plan/shape views.
6. Extend the original `RuntimePlanError`.
7. Migrate core plan consumers to admitted plan without deleting old raw
   execution APIs yet; those APIs must be internally unreachable from new
   constructors.

Exit: core plan/admission tests and workspace check green.

## G5 — AWBC codec and product admission

1. Add required private generation-contract field to `AwbcProgram`.
2. Change the version-1 codec directly; no old reader.
3. Include the contract in AWBC/product digest.
4. Implement standalone and plan-paired admission.
5. Implement AWBC typed-root inventory correlation.
6. Add non-Serde admitted product.
7. Make VM/fiber/product-step internal paths capable of consuming it.

Exit: core AWBC codec/verifier/admission/VM/fiber tests green.

## G6 — raw execution API cut

In one compile-clean workspace change:

1. change VM step/host step to admitted-only crate-private signatures;
2. change fiber construction/resume/restore;
3. change `AwbcProductStepExecutor`;
4. replace `ArcweftRuntimeExecutor` raw constructors and raw replacement;
5. change `Engine::{new,for_flow,for_entry}`;
6. replace ambiguous `BytecodeProgram` conversion names;
7. migrate all core tests/helpers;
8. add external trybuild and internal compile-closure tests.

No raw VM/fiber/product-step path remains after this gate.

Exit: core/executor tests and workspace check green.

## G7 — CharacterDialogue authority cut

1. Re-export/use lower typed declarations without adding higher dependencies.
2. Delete public arbitrary role-type construction.
3. Delete caller-supplied runtime custom digest constructor.
4. Add specialized generation admission and admitted Character/View wrappers.
5. Replace schema construction with `try_from_generation`.
6. Apply retained opaque tuple18/custom tuple2/direct inline-failure contract.
7. Apply exact nested voice and unique Choice semantics.
8. Migrate encode/decode/digest/equality/hash/normalize/clear/patch.
9. Delete root/custom/inline nominal side authority and old readers.

Exit: dialogue positive/negative/precedence/golden-focused tests green.

## G8 — driver, bundle, save, View, and front-end cut

1. Make generation image own one admitted aggregate and semantic identity.
2. Migrate session construction and hot swap.
3. Migrate restore, root/replay, and persistence before traversal.
4. Migrate View runtime/mount/restore.
5. Migrate bundle/AWFB decode-to-activation.
6. Migrate save/session snapshots.
7. Migrate native/Web/headless players.
8. Migrate agent/MCP/CLI.
9. Migrate runtime accelerator/JIT/AOT/codegen.
10. Delete bare runtime/image escapes and generation-blind success paths.

Exit: focused integration suites and workspace check green.

## G9 — parent A4 final unchecked nominal deletion

After every producer/consumer compiles through admitted authority, delete
together:

1. public `RuntimeNominalRecordValue::new`;
2. `RuntimeNominalRecordValue::validate_shape`;
3. descriptorless nominal normalize/clear/patch rebuilds;
4. identity/layout-only validators;
5. stale nominal test constructors;
6. `.1.2` self-authorizing producer-row types;
7. old raw plan/AWBC execution signatures;
8. compatibility aliases and dual readers.

Run external and internal compile-fail/compile-closure tests.

Exit: A4 is mergeable with no unchecked publication interval.

## G10 — full acceptance gates

Run at minimum:

```text
cargo fmt --all -- --check
cargo test -p arcweft-interaction-model --all-features
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-runtime-plan --all-features
cargo test -p arcweft-compiler --all-features
cargo test -p arcweft-core --all-features
cargo test -p arcweft-dialogue --all-features
cargo test -p arcweft-runtime-driver --all-features
cargo test -p arcweft-bundle --all-features
cargo test -p arcweft-save --all-features
cargo test --workspace --doc
cargo test --workspace --all-targets --all-features
cargo check --workspace --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
just structure-audit
```

Applicable Tier 2 for this authority/codec cut:

```text
cargo llvm-cov nextest --workspace --all-features --lcov --output-path target/llvm-cov/lcov.info
cargo miri test -p arcweft-core --all-features
cargo miri test -p arcweft-dialogue --all-features
cargo audit
```

Use repository-provided aliases when they are stricter. Record exact command,
toolchain, feature set, result, and skipped-platform reason in the implementation
note. A missing optional tool is not reported as green.

## G11 — parent A6 codec/golden/tamper closure

A6 performs only exhaustive version-1 codec, golden, corruption, and
cross-product audit:

- plan/AWBC identical contract bytes;
- AWBC generation section tamper;
- bundle/save/replay cross-generation tamper;
- CharacterDialogue opaque/voice/custom golden bytes;
- anonymous versus nominal bytes;
- no old reader/writer fixtures;
- every Arcweft-owned version exactly `1`.

A6 must not introduce authority, migrate an execution API, or retain an
unchecked path until then.

## Merge rule

G1-G8 may be staged locally, but no subset is accepted as the final A4 cut if a
new admitted authority coexists with a public unchecked publication path.
G9-G10 are the closure proof.
