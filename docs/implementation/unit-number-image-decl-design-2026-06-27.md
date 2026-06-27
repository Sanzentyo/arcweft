# Unit-number literals and image declaration body design notes — 2026-06-27

## Scope

This cut closes two concrete spec/implementation gaps found during the grammar audit:

1. `UnitNumber` suffix coverage in `docs/01-language/grammar.md` and
   `docs/01-language/literals-and-primitives.md`.
2. typed syntax ownership for `image` declaration bodies instead of CLI-only raw
   string splitting.

Macro/template/precompile and full Game Native UI component/AwaitView lowering
remain Advanced or presentation-layer work and are not part of this cut.

## Layering

- `arcweft-lang-syntax` owns lexical recognition and typed syntax AST only.
  It does not resolve units into runtime values.
- `arcweft-lang-sema` maps recognized unit suffixes to expected semantic
  primitive or named types.
- `arcweft-cli` consumes the typed image declaration body through public AST
  accessors. It does not reparse raw image body text as a compatibility
  fallback.

## Unit-number implementation

`UnitNumberSuffix` now models the documented non-duration suffix families:

- `%`
- length: `px`, `pt`, `em`, `rem`, `vw`, `vh`
- angle: `deg`, `rad`, `turn`
- audio/music: `db`, `lufs`, `bpm`, `bars`

`DurationUnit` now models documented duration suffixes:

- `ns`, `us`, `ms`, `s`, `min`, `h`

The numeric lexer also accepts underscore separators and radix-prefixed integer
forms (`0x`, `0b`, `0o`) so the literal examples in the language documentation
parse as typed syntax rather than fragmented path tokens.

## Image declaration implementation

`EntityDeclBody` gains `Image(ImageDeclBody)`. `ImageDeclBody` stores flat
assignment fields as `ImageDeclField { name, value_source, value }`.

The parser accepts dotted field names such as `alignment.x`, `playback.local_time`,
`transform.tx`, `param.role`, and `proxy.hit_test` because those are field names
in the documented flat image declaration surface, not nested expression paths.

CLI image declaration loading formats arguments from
`ImageDeclField::value_source()` and relies on the syntax parser to own image
body structure. No raw-line fallback remains in the CLI adapter.

## Verification

Focused validation run after applying this patch:

```bash
cargo fmt
cargo test -p arcweft-lang-syntax float_suffix_and_unit_number_literals_are_typed_syntax -- --nocapture
cargo test -p arcweft-lang-sema parses_surface_alias_and_resource_entity_families -- --nocapture
cargo test -p arcweft-cli parses_declared_image_object_args_and_default_id -- --nocapture
cargo test -p arcweft-runtime-plan strict_runtime_bracket_seq_folds_literal_values_to_dense_storage -- --nocapture
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structure audit scanned 1532 files and 838 Rust files and reported 0
errors and 105 existing warning-level hotspots.

## Structural Audit Notes

Repository state measured at Jujutsu change `xnwrorts`.

| Path | Bytes | LOC | Kind | Embedded test LOC | Responsibilities |
| --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-cli/src/app/image_declarations.rs` | 6489 | 197 | production plus unit tests | 40 | typed image declaration extraction for CLI/native capture image objects |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | 37510 | 985 | production | 0 | semantic helper mapping, including unit-number suffix type classification |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | 41752 | 1254 | unit-test module | 1254 | declaration parsing and semantic fixture coverage |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | 41784 | 1544 | production | 0 | syntax AST item declarations, including typed entity declaration bodies |
| `crates/arcweft-lang-syntax/src/expr.rs` | 60284 | 1826 | production plus unit tests | 461 | expression AST, lexer, literal parsing, and expression parser tests |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | 44899 | 1232 | production | 0 | item parser and structured declaration body parsing |
| `crates/arcweft-lang-syntax/tests/parser_p0.rs` | 16208 | 504 | integration test | 0 | parser smoke tests for language surface syntax |
| `crates/arcweft-runtime-plan/src/expr.rs` | 65753 | 1771 | production plus unit tests | 419 | runtime expression lowering and dense literal sequence folding tests |
| `crates/arcweft-runtime-plan/src/labels.rs` | 5390 | 140 | production | 0 | stable runtime labels and duration literal conversion |

`ast/items.rs`, `expr.rs`, `parser/items.rs`, and `runtime-plan/src/expr.rs`
are warning-level ownership hotspots but below error thresholds. This cut keeps
the changes local to their existing responsibilities rather than adding facade
re-exports or compatibility modules.

Largest workspace Rust files at this cut are unchanged seq-independent
hotspots:

| Path | Bytes | LOC | Note |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12394 | generated-like vertical orientation table |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255424 | 7445 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 225209 | 5838 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 5760 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209852 | 5285 | integration fixture suite |

## Known remaining gaps

- Advanced `macro` / `template` / `extern precompile mod` still need a separate
  design cut because they affect expansion, hygiene, source maps, build cache,
  and module item ownership.
- Game Native UI `AwaitView` and full component body lowering still need a
  presentation/HIR design cut. This patch only improves `image` declarations,
  which are already consumed by current CLI/native capture code.
