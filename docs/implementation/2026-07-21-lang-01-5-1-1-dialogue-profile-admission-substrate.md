# Lang-01.5.1.1 dialogue profile admission substrate

Date: 2026-07-21

## Outcome

This cut implements the non-executable typed substrate required before the
compatibility-free dialogue presentation authority switch:

- `HirModule::view_declarations()` and `HirTopLevelDecl::as_view()` expose the
  retained typed View inventory without source inspection;
- `AcceptedLaunchProfileInput` carries the exact accepted manifest, resolved
  profile, topology source revision, and immutable resource type registry into
  one compiler transaction; and
- `DialogueProfileRevision` retains the six typed admission facts shared by
  compiler, runtime-plan, save/reload, and tooling consumers.

No existing `dialogue defaults` path is copied into these types, and none of
the new inputs executes yet. The later authority cut must still move View
lowering into the compiler, admit the profile against the single validated
View/Style product, materialize the checked profile in runtime plans, and
delete the old source/defaults surface atomically.

## Ownership decision

The package places `DialogueProfileRevision` textually under
`arcweft-compiler`, but `arcweft-runtime-plan` must own the value while the
compiler already depends on runtime-plan. Keeping the type in the compiler
would invert the dependency graph or force a duplicate transport type.

The reusable six-field value therefore lives in `arcweft-dialogue`, beside
`DialoguePresentationProfile`, while catalog-aware construction remains a
compiler admission responsibility. This preserves the package's semantic
field contract without a facade, conversion helper, or compatibility wrapper.

## Direct evidence

- the HIR inventory filters by the typed `EntityDeclKind::View` and preserves
  declaration order;
- the launch input test proves the accepted manifest and resource registry are
  retained by the same `Arc`, not rebuilt from paths or TOML;
- the revision test proves all six nominal facts survive unchanged; and
- the compiler dependency direction remains acyclic.

## Remaining package work

- compiler-owned linked-HIR View lowering and one `ValidatedViewProduct`;
- catalog-aware `CheckedDialogueProfile::try_admit`;
- runtime-plan materialization of View, Style, inline-failure, and revision;
- CLI/project-loader/LSP/Agent reuse of the same compiled product;
- save/reload revision comparison; and
- atomic removal of `DialogueDefaultsItem`, selectors, raw runtime options,
  tooling projections, and tests for the removed surface.

These remain active work, not non-goals.

## Verification

- `cargo test -p arcweft-dialogue -p arcweft-lang-hir --no-fail-fast`:
  passed, including the new revision and typed View inventory tests;
- `cargo test -p arcweft-compiler
  accepted_launch_input_retains_one_exact_typed_object_graph --lib --
  --nocapture`: passed;
- `cargo check -p arcweft-dialogue -p arcweft-lang-hir
  -p arcweft-compiler --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo fmt --all -- --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/lang-01-5-1-1-admission-substrate-2026-07-21`:
  scanned 3,475 files, 1,807 Rust files, 832,655 Rust physical lines,
  and 94 manifests with 0 errors and 133 existing warnings.

The changed production hotspots are
`arcweft-lang-hir/src/model.rs` at 28,503 bytes / 1,090 physical lines,
`arcweft-compiler/src/project/registration.rs` at 8,224 bytes / 259 lines,
and the new `arcweft-dialogue/src/presentation_revision.rs` at 4,625 bytes /
124 lines. The audit records dependency fan-in/fan-out as 12/5 for
`arcweft-lang-hir`, 4/21 for `arcweft-compiler`, and 5/11 for
`arcweft-dialogue` after removing the compiler's duplicate dev dependency.
The two new normal edges into the compiler are the lower launch and resource
models that its admission transaction must consume; no lower crate depends on
the compiler.
