# Lang-01.5.1.1 compiler View-product ownership — accepted-product cut

## Basis and completion boundary

This cut follows the corrected
`arcweft-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction-final-contract(1).zip`
package. It completes the compiler-ownership portion of migration Step 6. It
does **not** claim that dialogue-profile admission or the complete
Lang-01.5.1.1 sequence is finished.

The accepted design difference is that a compiler View product has two
explicit source revisions:

- `authored_source_revision` covers the exact authored project module
  documents;
- the `ValidatedViewProduct` source revision covers the complete product
  source set, including the engine-generated standard dialogue View and Style
  documents.

Dropping standard provenance merely to make these revisions equal would leave
`std.view.dialogue` without the non-optional source owner required by profile
admission and tooling. Later admission therefore compares against the complete
product revision while separately proving that every authored module is
represented exactly.

## Implemented in this cut

- Moved View layout, schema, validation, and lowering responsibility from
  `arcweft-cli` to `arcweft-compiler` without introducing another parser or
  View catalog.
- Made `CompiledProject` the production owner of the accepted typed Style
  artifact, `CompiledImageCatalog`, Arc Fx definitions, and
  `CompiledViewProduct`. Project compilation now executes explicit Style,
  Image, Fx, and View lowering stages and supplies the exact accepted
  `ResourceTypeRegistry` from its immutable compilation context to View
  admission.
- Made View declarations structured once in syntax as `ViewDeclBody`; sema,
  project indexing, and compiler lowering consume that typed body rather than
  reparsing `signature_tail`. Missing-body recovery retains the typed owner.
- Preserved malformed Await/Match branches and modifiers through recursive
  recovery instead of silently dropping them. Reserved expression heads can no
  longer be reinterpreted as executable View calls, and unsupported dynamic
  text, boolean, layout, handler, callback, and Await-source forms are rejected
  structurally rather than compiled from `Debug` text.
- Bound every retained View node, branch, and ranged modifier to its exact
  authored range. Missing `for ... key = ...` punctuation and malformed,
  unknown, or duplicate navigation arguments now retain non-executable
  recovery instead of silently weakening the authored node.
- Made canonical type labels precedence-aware and expanded View callable
  semantic identity to the complete typed signature contract, including
  parameter arity/default/rest semantics, default expressions, generics,
  bounds, where clauses, and return type. Formatting-only changes remain
  identity-stable.
- Added `HirModule::view_declarations` consumption so every typed View is
  lowered. The former mount/defaults suffix discovery and filtering path was
  removed.
- `ViewProjectLowerer::for_project` creates one `ValidatedViewProduct` from
  authored and standard typed View/Style resources inside the project
  compilation transaction. Production CLI bundle and runtime-profile
  preparation consume the product retained by `CompiledProject` and do not
  lower a second View product.
- Added an inherent nominal `ViewId` lookup on
  `ValidatedViewProgramResource`.
- Lowered Image declarations once from retained `ImageDeclBody` into a
  compiler-owned catalog. Required fields, nominal reference families,
  declaration/field ranges, and collisions are checked at this boundary; the
  old CLI source reparse and string-argument projection were deleted.
- A missing Image asset is emitted as a source-bound structured diagnostic
  with the declaration as the primary range. Bundle source maps keep the root
  module as the explicit primary document instead of relying on map order.
- Removed the CLI's second project-document SourceMap assembly. Runtime-profile
  and bundle assembly now consume the exact SourceMap retained by the
  compiler-owned accepted View product, including its generated standard
  sources.
- Lowered Arc Fx definitions once from linked HIR and retained the typed
  inventory in the compiler candidate.
- Required standalone lowerer inputs to use HIR bound to the exact supplied
  `SourceDocumentIdentity`; a same-length detached document is rejected.
- Retained exact declaration spans for authored View and Style identities.
- Added generated source ownership for `std.view.dialogue` and retained the
  existing generated standard Style ownership in the complete product source
  map.
- Removed post-validation extension of a `CompiledViewProduct`. Loose external
  program/Style sidecars cannot be merged into the compiler-owned accepted
  candidate after validation.
- Deleted production ingestion of `view.program.json`, `view.style.json`, and
  the loose `content/view.theme.json` sidecar. Absence of an authored theme now
  selects the runtime default; it does not manufacture a second authoring
  authority.
- Deleted the dead `samples/reactive-view-style/content/view.text.json`
  sidecar and updated the sample to document that all View authoring comes
  from its `.arcw` source.
- View-local input handles are compile-time typed bindings. They no longer
  fabricate scalar `BindLocal` programs from symbolic expression text.
- Closed all retained text-control and Scroll policy enums at the compiler
  boundary. Defaults apply only when a value is absent; an authored unknown
  symbol is a structured `UnknownPolicySymbol` failure.
- Canonicalized direct-source package identity before project compilation so
  RichText Fx applications and the retained Fx definition inventory use the
  same `local.arcweft.*` owner. This fixes a pre-existing mismatch exposed by
  the complete bundle/View test route; it is not a compatibility alias.
- Added a bundle serialization projection from the validated product. The
  serializable bundle clones canonical sections; compiler/runtime/tooling Arc
  identity remains a later transaction boundary and is not represented as
  pointer identity in bundle bytes.

## Explicit remaining work

1. Integrate the AW-AH-009.3 callable-catalog cut. Project compilation currently
   reaches that independent registration failure before all transaction tests
   can exercise Image/View admission. After integration, remove the temporary
   ignore from the typed Image-admission discard test and run the full project
   compilation suite.
2. Once AW-AH-009.3 supplies the production project-compilation fixture path,
   migrate the remaining compiler and CLI tests off the test-only
   `ViewProjectLowerer::for_source` entry point. Then remove that entry point,
   narrow the lowerer/error visibility to the owning crate, and delete the
   corresponding `#[cfg(test)]` CLI projection. There is no production caller
   or second production View authority left in this cut.
3. Implement `CheckedDialogueProfile::try_admit` and the later runtime-plan,
   LSP, reload/save, old-defaults deletion, and profile-product integration
   steps from the corrected package.

## Verification

Completed while developing this cut:

```text
cargo test -p arcweft-lang-syntax --test view_callable
  PASS — 3 tests

cargo test -p arcweft-compiler --test view_product
  PASS — 5 tests; typed View ownership, source binding, recovery, and
  unsupported executable forms

cargo test -p arcweft-compiler --test image_catalog
  PASS — 8 tests; typed fields, nominal identities, exact ranges, and
  collisions

cargo test -p arcweft-bundle --all-features
  PASS — unit and integration suites, including View-product validation,
  SourceMap ownership, and standard-dialogue resources

cargo test -p arcweft-cli --test responsive_stage_placement \
  missing_image_asset_is_emitted_as_a_source_bound_structured_diagnostic
  PASS — 1 test

cargo test -p arcweft-cli --lib app::bundle::tests
  PASS — 47 tests; compiler-owned View/Image/Fx products, runtime mount
  separation, and direct-source package identity are exercised

cargo check -p arcweft-cli --all-targets
cargo check --workspace --all-targets --all-features
  PASS

cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS

cargo build -p arcweft-cli --bin arcw
  PASS

CARGO_BUILD_JOBS=2 just test-workspace
  BLOCKED — compilation and all preceding suites completed without the earlier
  Windows paging-file failure; arcweft-compiler then ran 98 tests, passed 85,
  and failed 13. Every failure resolves to the already isolated
  aw.callable.catalog.registration / CorruptCallableCatalog blocker.

just reactive-view-style-sample
  PASS — compiler-owned `.arcw` View/Style authoring produced the AWFB with no
  loose sidecar. The stale call to the already removed renderer showcase was
  deleted from the recipe.

just test-tier2
  PASS — 22/22 MCP stdio E2E tests plus every subsequent animated-image,
  native raw capture, vertical/ruby/text-combine, and checked-in golden group.

cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/lang-01-5-1-1-view-product-compiler-move-2026-07-21
  PASS — 3,479 files, 1,808 Rust files, 835,909 Rust physical LOC,
  94 manifests, 0 errors, 134 warnings.

.\target\debug\arcw.exe run --runner native \
  .\samples\modern-feedback-view\src\main.arcw
  MANUAL SMOKE PASS — the native player stayed open and responsive; the former
  `image.glass_bg` missing-asset failure did not recur
```

`cargo fmt --all -- --check` and `git diff --check` also pass at this cut. The
only new changed-file size warning is
`crates/arcweft-compiler/src/view/lowering.rs` at 1,229 physical LOC. It remains
the orchestration layer over already separated `content`, `modifiers`,
`scroll`, and `text_controls` responsibility modules; it introduces no new
cross-layer dependency or embedded test module. It should be split again when
the test-only standalone lowerer is removed after AW-AH-009.3 rather than by
creating a transitional wrapper now. The dependency direction remains compiler
toward renderer-independent presentation/resource types, never toward runtime
or tooling.

The full `arcweft-compiler` unit suite currently exposes an independent
`aw.callable.catalog.registration` / `CorruptCallableCatalog` failure in
project-compilation tests. This failure is not hidden or counted as
View-product acceptance. It belongs to the active AW-AH-009.3 callable-catalog
completion and must pass after that cut is integrated before the overall goal
can complete.

## Prohibited tactics retained

This cut adds no CSS/Takumi path, source gate, removed-spelling recognizer,
compatibility alias, dual reader, deprecated API, or source-text View
rediscovery. The removed CLI image/source and loose sidecar paths were deleted
rather than retained as compatibility surfaces.
