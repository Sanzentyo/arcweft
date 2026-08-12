# Compile-clean deletion-driven implementation order

## Global rule

Every gate is compile-clean. The two descriptor schema cuts may be separate
commits, but no schema-1 success path survives its owning cut. Gates G3a and G3b
are a protected atomic merge group and are merged/released/cherry-picked only
together; no producerless successful catalog interval is permitted.

## G0 — pin and inventory

- verify full Git SHA `78f50f5b5ac082745bab91b7373a6602918a436d`;
- preserve any unrelated dirty A1 work by using a separate Git worktree;
- run the exact symbol/fixture closure commands in `INVENTORY.md`;
- capture existing schema-1 and digest goldens before replacing them;
- make no production behavior change.

Exit: reviewed inventory maps every match to a target gate; no unexplained
direct `ArcweftRustTypeDecl`/`AdapterNominalDeclaration` literal remains.

## G1 — Rust ABI schema 2 hard cut

- add `producer.rs`, validated ID/error, exports, tests;
- add mandatory field/accessor to `ArcweftRustTypeDecl`;
- set schema constant/builder to 2;
- add JSON header preflight/private DTO/typed errors;
- order programmatic validation as specified;
- migrate display/tests/direct literals;
- update rust-abi-build deterministic JSON/hash golden;
- extend `ArcweftType` macro helper parsing and all trybuild pass/fail fixtures;
- delete schema-1 reader/writer/goldens and derive success without the helper.

Exit: rust-abi, rust-abi-build, macros, trybuild, format, workspace check, and
Clippy are clean. No Rust type declaration can be constructed or decoded
without a validated producer.

## G2 — adapter-context schema 2 hard cut

- add adapter-owned ID/error in `manifest::nominal` and re-exports;
- add mandatory declaration field/constructor/accessor;
- set adapter schema to 2;
- add JSON custom header preflight and TOML value preflight;
- add global required/spelling/reserved passes and private schema-2 DTO;
- migrate standard manifests, direct literals, JSON/TOML fixtures, and goldens;
- explicitly author `arcweft.adapter.native-http` for `HttpRequestContext`;
- explicitly author one shared `arcweft.adapter.inference-tensor` producer for
  `Conv2dApi`, `InferApi`, and `TensorF32`;
- delete schema-1 readers/writers/defaults/goldens.

Exit: adapter-context tests and public API checks are clean; no external
adapter-native row can omit or reserve a producer.

## G3a/G3b — protected publication switch

### G3a adapter-sema staging

- extend generated-source v2 grammar/source map;
- add private `ExternalOpaqueProducer` enum with inherent projection;
- add typed source-bearing errors;
- precompute all adapter-native and Rust producer conversions;
- add producer to every `AcceptedNominalInventoryInput` construction;
- bump environment-manifest digest to v2 and update goldens.

### G3b lang-sema accepted owner switch

- make inventory producer mandatory;
- use parent producer-bearing `AcceptedNominalSemantics::Opaque` and
  `try_new_opaque` APIs;
- preserve producer in instantiation and substitution;
- bump accepted catalog digest to v2;
- explicitly keep semantic identity digest declaration+arguments only;
- delete producerless opaque variant/constructors and runtime-facing
  producerless success path.

Protected-group exit: adapter-sema and lang-sema focused suites, digest vectors,
instantiation/substitution tests, compiler entry tests, workspace check, and
Clippy pass together. Neither subgate is released alone.

## G4 — downstream fixture and artifact migration

Migrate standard/desktop Rust exports, project loader manifests, compiler/LSP/
verify-LSP fixtures, generated source snapshots, digest snapshots, examples,
and all maintained JSON/TOML. Fixture producer literals follow
`FIXTURE_PRODUCER_CATALOG.md`; none is inferred from production metadata.

Exit: project-loader, compiler, LSP, verify-LSP, desktop/adapter integration,
and all maintained artifact/golden suites pass.

## G5 — deletion and structural closure

Delete every item in `DELETION_SET.md`. Run exact repository searches proving:
no schema-1 constants/goldens/readers, no producerless direct construction, no
optional/default producer, no producer-to-name/path/hash derivation, no
admission field, no side table/registry/callback/trait, no temporary overlay,
and no old generated-source header/domain.

Exit: source audit, dependency/layer audit, format, workspace check, Clippy with
warnings denied, and the repository's checked-in fast verification pass.

## G6 — resume parent A1.2

Only after G5 passes may parent A1.2 consume the mandatory producer-bearing
accepted catalog for checked-type projection. Parent A1.3/A1.4 remain ordered
as defined by the parent package; this correction does not change their ABI,
codec, tag, or save decisions.

## Planned validation commands

These commands are implementation gates, not claims about this design return:

```text
cargo fmt --all -- --check
cargo test -p arcweft-rust-abi --all-targets --all-features
cargo test -p arcweft-rust-abi-build --all-targets --all-features
cargo test -p arcweft-rust-abi-macros --all-targets --all-features
cargo test -p arcweft-adapter-context --all-targets --all-features
cargo test -p arcweft-adapter-sema --all-targets --all-features
cargo test -p arcweft-lang-sema --all-targets --all-features
cargo test -p arcweft-project-loader --all-targets --all-features
cargo test -p arcweft-compiler --all-targets --all-features
cargo test -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-verify-lsp --all-targets --all-features
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 just verify
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check -- .
test -z "$(git diff --name-only --diff-filter=U)"
```

If exact crate package names differ at the implementation head, use the
maintained package names discovered by `cargo metadata`; do not silently omit
the corresponding surface.
