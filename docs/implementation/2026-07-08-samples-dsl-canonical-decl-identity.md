# Samples DSL canonical declaration identity update

## Source package

`D:/sanze/Downloads/arcweft-samples-dsl-update.zip`

The package requested sample DSL updates from generated-style explicit
declaration identities to compact canonical declaration names while preserving
public references such as `@flow.opening` and `@frag.intro`.

## Applied scope

- Rewrote affected sample `flow` declarations from `flow @flow.* name` to
  `flow name`.
- Rewrote `pub fragment @frag.intro intro: FlowFragment` to
  `pub fragment intro: FlowFragment`.
- Preserved existing reference sites such as `include @frag.intro`,
  `goto @flow.opening`, and `signal.set(@signal.current_flow, ...)`.
- Followed `arcw check` hints by compacting sample `signal` and `metric`
  declarations in the touched sample family.

## Implementation finding

Running `arcw check samples/agent-script/native-project-graph-relations.arcw`
after the package overlay exposed an implementation bug: compact fragment
declarations normalized to `fragment.intro`, while references and existing
public contracts use the canonical public ID family `frag`.

The fix makes `FlowKind::Fragment` own the rule that the declaration keyword
`fragment` emits public IDs in the `frag` family. Parser validation and HIR
lowering now share that rule, and HIR flow slug extraction strips both `frag.`
and the legacy `fragment.` prefix.

## Validation notes

The updated samples passed `arcw check` after rebuilding `arcw`:

```bash
target/debug/arcw.exe check samples/visual-novel-mini/src/tool.arcw
target/debug/arcw.exe check samples/visual-novel-mini/src/server.arcw
target/debug/arcw.exe check samples/visual-novel-mini/src/game.arcw
target/debug/arcw.exe check samples/visual-novel-mini/tests/opening.arcw
target/debug/arcw.exe check samples/visual-novel-mini/benches/opening.arcw
target/debug/arcw.exe check samples/agent-script/native-visual-regression.arcw
target/debug/arcw.exe check samples/agent-script/native-project-index.arcw
target/debug/arcw.exe check samples/agent-script/native-project-graph-relations.arcw
target/debug/arcw.exe check samples/agent-script/native-choice-dispatch.arcw
target/debug/arcw.exe check --manifest-path samples/zundamon-awchar/arcw.toml --profile dev
```

`samples/zundamon-awchar/src/main.arcw` still fails when checked directly
because direct source checks do not load launch-profile `character_manifests`.
The profile-backed project check above passes and remains the intended
validation route for that sample.

Focused Rust validation:

```bash
cargo test -p arcweft-lang-sema --lib fragment -- --nocapture
cargo test -p arcweft-lang-sema --lib flow_relative_decl_ids_normalize_like_implicit_names -- --nocapture
cargo test -p arcweft-lang-sema --lib project_index_records_entry_and_flow_entity_relations -- --nocapture
cargo fmt -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -- --check
cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-cli --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit scanned 2444 files, 1170 Rust files, and 573192 Rust
physical LOC, reporting 1 existing error and 148 warnings.

`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema
--all-targets -- -D warnings` is blocked by existing
`clippy::large_enum_variant` diagnostics in
`crates/arcweft-lang-syntax/src/ast/items.rs` for `TraitMember` and
`ImplMember`; those diagnostics are outside this DSL update.
