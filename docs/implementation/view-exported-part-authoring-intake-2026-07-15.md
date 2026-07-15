# View exported-part authoring intake

- Date: 2026-07-15
- Package: `arcweft-seq-06.11d.2.1.1-view-exported-part-authoring-final-contract.zip`
- Baseline parent: Git `8140470a` / Jujutsu `wnlrvplr`
- Working change at intake: `ywsqqorx`
- Status: partial implementation; package acceptance is not complete

## Completion boundary

The returned d.2.1.1 package is already recorded as not accepted for production
implementation by the production-reconciliation request. This cut implements
the portions whose behavior is closed by the package and current typed owners.
It does not reinterpret the unresolved production boundaries as implementation
choices merely because a local shape compiles.

Implemented in this cut:

- one canonical `export part local as public` leading View-body declaration;
- typed `.part(local)` syntax, exact typed name ranges, structured recovery,
  duplicate-modifier rejection, and removal of the `part = ...` lowering path;
- per-module HIR owners with module, visibility, owner range, private targets,
  target family, occurrence shape, and authored exports;
- checked owner/local/public namespaces, deterministic local IDs, diagnostics,
  explicit no-re-export validation, and typed export provenance;
- fallible `ViewProgramBuilder` export validation and separate local/public name
  newtypes;
- compiler projection from checked exports into owner-qualified typed product
  records for the current single-source product assembly path;
- typed bundle records, canonical ordering, indexed owner/target validation,
  source-reference structural validation, SourceMap extent validation, merge
  remapping, rejection of the provisional flat record, and complete-product
  validation before runtime conversion;
- normal CLI `.arcw` lowering of a non-empty typed export inventory;
- an immutable accepted-program wrapper and one derived runtime capability
  table used by mounted View style facts;
- direct-boundary runtime regression coverage proving no accidental
  two-boundary traversal;
- formatter edits which canonicalize only fully parsed declarations/modifiers,
  preserve malformed text, preserve declaration order, and are idempotent; and
- a shared typed LSP metadata index for owner-local/public hover, definition,
  references, and legal export-local completion. Local and public symbols stay
  disjoint.

## Required residuals

The following are requirements of the package but are not complete and must
not be reported as implemented:

| Residual | Why this cut cannot choose it safely | Follow-up |
| --- | --- | --- |
| mandatory multi-module source identity, multi-source SourceMap metadata, compiler instruction-part index, and explicit link rewrite maps | the current product has one `BundleSource`, while proof 01.1 will remove linked-HIR flattening; choosing a duplicate source registry now would conflict with both owners | [d.2.1.1.2](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.2-view-part-source-link-contract.md) |
| canonical product-to-runtime owner/program/part identity and fully typed boundary/occurrence authority | the current runtime still derives some identity from resource strings and has no final owner-ID/occurrence allocation contract | [d.2.1.1.3](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.3-view-part-runtime-authority-contract.md) |
| application-edge-backed Style-part bindings and complete LSP rename/symbol/token behavior | checked call/application graph ownership and multi-document atomic metadata are not present; text matching would violate the privacy contract | [d.2.1.1.4](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.4-view-part-contextual-tooling-contract.md) |
| fingerprints, revisions, transactional accepted-program replacement, mounted reconciliation, and targeted invalidation | the runtime has no final replacement transaction or shared revision owner, and d.4.2 may add an adjacent environment revision | [d.2.1.1.5](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.5-view-part-hot-reload-contract.md) |

The throwing order is `.2` first after proof 01.1. After `.2` is final, `.3`
and `.4` may be designed in parallel by separate assignees. `.5` is last and
must consume the final `.3`, `.4`, and any landed d.4.2 revision/evidence types.

## Proof 01.1 migration boundary

The preserved authored-part interface is the per-module `HirViewPart*`
inventory and its owner module assignment. Existing project flattening is only
the checkout's current aggregation mechanism. No compatibility wrapper,
alternate append API, or dependency on `HirModule::append_module_body` was
added. When proof 01.1 lands, aggregation must move mechanically to
`HirProjectView` iteration and the old append/linked flattening may be deleted.

## Validation evidence

All Cargo commands used:

```text
CARGO_TARGET_DIR=D:\git\arcweft-targets\exported-part
CARGO_INCREMENTAL=0
```

Completed:

- `cargo test -p arcweft-lang-syntax --test view_export_part` — 5 passed;
- `cargo test -p arcweft-lang-sema --test view_part` — 4 passed;
- `cargo test -p arcweft-tooling --test view_export_part` — 2 passed;
- `cargo test -p arcweft-lsp view_part` — 2 passed;
- `cargo test -p arcweft-bundle --test view_resource_codecs exported_part` — 6 passed, including the old-flat-shape rejection;
- `cargo test -p arcweft-runtime-driver --test view_runtime exported_part` — 1 passed;
- `cargo test -p arcweft-cli authored_export_part_lowers_to_typed_product_inventory` — 1 passed;
- `cargo check -p arcweft-cli --all-targets` — passed;
- `cargo check -p arcweft-runtime-driver --all-targets` — passed; and
- targeted all-target/all-feature clippy for every changed production crate,
  with `-D warnings` — passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 2,758
  files scanned, 1,317 Rust files, 638,479 Rust physical LOC, 90 manifests,
  0 errors and 128 warnings.

One aggregate focused-test command exceeded the 300-second shell budget while
relinking the LSP test graph. Every preceding focused command in that
stop-on-failure sequence had passed, and the same LSP tests had already passed
individually. This timeout is reported as incomplete aggregate evidence, not a
test failure and not a workspace-test claim.

Full `just test-workspace`, doc tests, and Tier 2 suites were not run in this
isolated package cut. The final integrated main cut must select them according
to `docs/implementation/test-execution-policy.md`.

## Structural notes

Responsibility modules were added for View identities, syntax AST/parser,
HIR, sema catalog/check/diagnostics, compiler projection, bundle model/codec,
runtime acceptance, formatter, and shared LSP metadata. Existing warning-level
owners remain visible rather than being hidden by compatibility modules:

- `arcweft-bundle/src/resource_codec/view/model.rs` and `codec.rs` remain near
  the production error threshold, but exported-part code is in `model/part.rs`
  and `codec/part.rs`;
- syntax View AST/parser owners remain above 1,200 LOC, but feature logic is in
  `ast/view/part.rs` and `parser/view/part.rs`; and
- runtime evaluation was not expanded with replacement orchestration; accepted
  part authority is in `view_runtime/part.rs`.

The structural warnings are pre-existing/threshold review signals plus the
current exact checkout measurements; no error-level exception was reported.
Final Jujutsu status and diff statistics are recorded in the handoff.
