# Compile-clean implementation and deletion order

No intermediate state is an accepted contract checkpoint. The A4 work may be
developed in local compile-clean steps, but it is merged/returned only after
the final deletion gate is green. There is no release or compatibility window
with both authorities.

## G0 — repin and evidence

1. Materialize a clean checkout at
   `98ccafa5f0113a50f8a0f5e985df5f695c401588` or mechanically reconcile a
   descendant.
2. Record `git rev-parse HEAD`, `git status --short`, and parent ZIP hashes.
3. Re-read root and every scoped `AGENTS.md` covering touched files.
4. Run the retained A1–A3 focused suites before editing.
5. Inventory all `RuntimeNominalRecordValue::new`, `validate_shape`, dialogue
   `RuntimeFieldPath`, typed-value Deserialize, and nominal rebuild call sites.

Exit: no production change; exact baseline and discovery inventory recorded.

## G1 — close existing variant predicate defect

1. Extend the original `RuntimeCheckedType::accepts_value` `Variant` branch to
   check owner, ordinal, case name, payload presence, and child type.
2. Add exact positive/negative cases for nominal variants, Result, Option, and
   inline failure.
3. Do not add a helper trait/free validator.

Exit: focused core tests, workspace check, Clippy, and rustfmt green.

## G2 — catalog declarations, errors, and operational handles

1. Add catalog key/declaration/producer declaration to the existing core
   nominal-record module.
2. Add catalog/lookup/tree errors to that owner.
3. Add operational catalog, producer capability, and value handle with private
   fields and no Serde/public constructors.
4. Implement `try_construct` by calling the existing crate-private
   `try_from_accepted_layout`.
5. Add compile-fail tests proving handles cannot be constructed and arbitrary
   layouts/scalars cannot publish values.

The old public nominal constructor still exists only as branch-local migration
scaffolding; this gate is not mergeable independently.

## G3 — whole-plan admission and runtime-plan projection

1. Add the private catalog declaration field and `try_with_nominal_record_catalog`
   to `RuntimePlan`.
2. Add `AdmittedRuntimePlan` and consuming `RuntimePlan::try_admit`.
3. Extend the existing `RuntimePlanError` enum in `plan::entry_inventory`.
4. Retain the semantic-facts nominal `Arc` interning map and emit one canonical
   generation declaration.
5. Compute external producer authorization keys from accepted closed checked
   type facts, including CharacterDialogue role/custom types.
6. Reject duplicate/conflicting/missing/unreachable/wrong producer rows.
7. Change runtime construction APIs to require `AdmittedRuntimePlan` where
   practical; no runtime executes a merely verified raw plan.

Exit: core/runtime-plan/compiler focused tests and workspace gates green.

## G4 — migrate core producers and consumers

1. Pure evaluator and structured engine obtain project handles for every
   nominal expression before construction.
2. AWBC verifier/lowering/VM/fiber carry canonical catalog keys or admitted
   handles; no raw value constructor remains.
3. Pattern, root, replay, ownership, nesting, and snapshot ingress validate
   against the active catalog before traversal.
4. Runtime codegen either consumes the admitted path or returns its existing
   typed unsupported error; it never falls back to anonymous/unchecked.
5. Preserve A1–A3 authored-order scatter, defining order, IDs, and bytes.

Exit: focused core/AWBC/root/replay tests and workspace gates green.

## G5 — replace CharacterDialogue physical representation

1. Add role checked types and change custom descriptors to one
   `RuntimeCheckedType`.
2. Change schema construction to require the admitted
   `std.character_dialogue` producer capability and preflight every type.
3. Remove root nominal/layout inputs and `CharacterDialogue.layout`.
4. Encode/decode the exact opaque 18-tuple.
5. Replace custom-entry nominal records with sorted tuple2 entries.
6. Store inline failure as the direct closed variant.
7. Remove `Dynamic`, root/custom/inline nominal IDs/layout functions, and old
   nominal wrappers in the same gate.
8. Move digest/canonical encoding to the schema owner.

Exit: dialogue positive/negative/precedence tests and workspace gates green.

## G6 — live typed values and descriptor-aware transformation

1. Remove direct Deserialize and raw public constructor from
   `CharacterDialogueTypedValue` and role/custom wrappers.
2. Add schema-owned role/custom admission methods.
3. Replace descriptorless normalization with expected-type/catalog-aware
   recursion.
4. Replace empty/clear with checked-type-directed behavior.
5. Replace `RuntimeFieldPath` with `RuntimeValuePath` in `StructuredPatch`.
6. Implement all-path preflight, deterministic mutation, nominal rebuild on
   unwind, full revalidation, and atomic publication.
7. Delete `replace_runtime_value` and every raw struct-update bypass.

Exit: normalize/clear/patch matrices green, including atomic late failure.

## G7 — driver, View, bundle, save, root, replay, and adapters

1. Raw bundle plan -> `try_admit` before generation/session construction.
2. Session activation and hot swap accept only admitted plans.
3. View input/state/restore validates before mount/traversal.
4. Save/session restore maps active slot types, validates, then traverses.
5. Root/replay values validate before transition/replay.
6. CharacterDialogue wire values decode only through active schema.
7. Agent/CLI/headless/native/Web/runtime-accelerator projections inspect
   admitted values and do not reproduce nominal values.
8. Add typed source-preserving variants at driver/save/replay boundaries;
   remove identity/layout/type string flattening.

Exit: driver/View/bundle/save/root/replay focused suites green.

## G8 — final A4 deletion cut

After all producers/consumers compile through the selected authority, delete in
one workspace state:

1. public `RuntimeNominalRecordValue::new`;
2. `RuntimeNominalRecordValue::validate_shape`;
3. every descriptorless reconstruction branch and identity/layout-only
   validator;
4. old CharacterDialogue root/custom/inline nominal helpers and fields;
5. typed-value nominal/layout side scalars and Deserialize;
6. dialogue `RuntimeFieldPath` and ordinal traversal;
7. stale test constructors/helpers and any compatibility aliases.

Run compile-fail tests from both external trybuild crates and internal obsolete
call-site fixtures. Search is discovery evidence; compile/test success is the
closure proof.

Exit: parent A4 is complete and mergeable; no unchecked publication interval.

## G9 — final A4 verification

Required minimum commands (use repository-provided exact aliases when they are
stricter):

```text
cargo fmt --all -- --check
cargo test -p arcweft-core --all-features
cargo test -p arcweft-runtime-plan --all-features
cargo test -p arcweft-dialogue --all-features
cargo test -p arcweft-runtime-driver --all-features
cargo test -p arcweft-bundle --all-features
cargo test -p arcweft-save --all-features
cargo check --workspace --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo clippy --workspace --all-targets --all-features -- -D warnings
just structure-audit
```

Also run every repository-required applicable Tier 2 command from the exact
commit's test policy. Record commands and results in the implementation note.

## G10 — parent A6 codec/golden closure

1. Update exact opaque CharacterDialogue golden fixtures.
2. Prove anonymous/ordinary nominal bytes remain unchanged/distinct.
3. Complete AWBC/bundle/save/root-replay tamper and cross-product fixtures.
4. Audit every version field and prove it remains `1`.
5. Confirm no old reader/writer or compatibility fixture remains.

A6 is not allowed to defer an A4 validation or deletion.
