# Fx function presentation graph implementation — 2026-07-11

## Implemented cut

Arcweft's provisional `decoration` declaration has been replaced directly by
the single authoring surface `#[fx] fn ... -> Fx`. There is no compatibility
alias for `decoration`, `[decorate]`, `#[text_motion]`, `#[text_effect]`, or
`#[text_shader]`.

This cut implements:

- shared `FnParam` defaults, with Fx defaults restricted to closed values;
- argument-free `#[fx]` validation, implied purity, one non-generic parameter
  group, simple typed identifiers, named-only calls, and no rest parameters;
- typed `FxId`, `FxInstanceId`, ABI hash, semantic hash, definition, graph,
  node, property, and value boundaries;
- validated Fx package/qualified-name components at construction and serde
  decode boundaries, plus canonical typed ABI/semantic hashing independent of
  Rust `Debug` output and named-property order;
- deterministic graph composition, cycle detection, and expansion budgets;
- static style/text/color plus transform, mask, filter, shader, transition,
  conditional, and ordered stack graph nodes;
- View `.fx(...)` parsing, validation, bundle instruction lowering, retained
  instance identity, and runtime-host sidecar preservation;
- canonical View Fx argument ordering, duplicate/name validation, and an
  aggregate bundle decode budget;
- RichText `[fx call(...)]...[/fx]` validation, closed argument binding, and
  expansion of supported static text/color/style nodes;
- typed `DialogueTagKind` classification for `fx`, `reset`, point, span, and
  other tags instead of repeated raw-string comparisons;
- removal of the former source-local scalar exporter/registry bridge;
- grammar, samples, LSP/tooling fixtures, and `just test-rich-text` updates.

Unit-bearing arithmetic is intentionally conservative. The checker permits a
dimensionless float to scale a unit value and permits division of a unit value
by a float. It does not infer `Length / Length -> f32` or add/subtract unit
values merely because both erase to the same `Length` type: Arcweft does not yet
carry the unit normalization evidence needed to make `px / em` or `px + em`
sound. Unsupported `Duration` arithmetic is also rejected because the runtime
evaluator does not implement it.

## Explicitly incomplete integration

The following work is not represented as complete by this cut:

- `FxDefinition` graphs are not yet a first-class bundle section consumed by
  View/native/Web renderers. View bundles retain the resolved definition ID and
  reactive argument schemas, but do not execute the graph.
- Typed sampler closures such as `|ctx| Transform2D { ... }` are semantically
  checked, but lowering currently replaces the closure with a non-executable
  expression label; no typed sampler program, capture schema, or renderer
  evaluator is retained yet. Dynamic RichText leaves preserve a complete Fx
  identity and remain observably unresolved instead of falling through to an
  unrelated built-in with the same basename.
- Known closed View/RichText argument values are checked against Fx parameter
  types, but reactive View expressions still need the runtime binding schema.
  Constructor property schemas beyond the currently checked color and sampler
  contracts need a closed typed inventory shared by sema and renderers.
- Linked-module `use`/`pub use` resolution does not yet retain enough original
  declaration identity in HIR to guarantee alias-stable `FxId` resolution.
- External Rust/WASM `#[fx]` declarations require the same unresolved renderer
  ABI and package symbol work.

These gaps are split into independently actionable requests:

- [Executable Fx graph and renderer ABI](../reviews/requests/2026-07-11-seq-06.16.9.1-fx-executable-graph-and-renderer-abi.md)
- [Fx package symbol and re-export identity](../reviews/requests/2026-07-11-seq-06.16.9.2-fx-package-symbol-and-reexport-identity.md)

## Verification

Passed:

- `cargo fmt --all --check`
- `just --unstable --fmt --check`
- `git diff --check`
- `cargo run -p arcweft-cli --quiet -- check samples/rich-text-fx.arcw`
- focused Fx tests for syntax/HIR/sema/presentation/runtime-plan/View/bundle
- `just test-rich-text`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/fx-function-presentation-graph-2026-07-11`
  (0 errors, 149 warnings after the required `checker/expr.rs` split)

`just test-workspace` was attempted but could not finish on this Windows host:
linking exhausted the paging file and available link/PDB storage (`os error
1455`, `LNK1318`, `LNK1180`, and `LNK1140`). The focused changed-crate tests,
the richer native route, workspace check, and workspace clippy all completed.
The failure is environmental rather than a failing Rust test assertion.
