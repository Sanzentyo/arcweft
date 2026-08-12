# Producer, consumer, fixture, and deletion inventory

This inventory names every known owner and defines an exact repository-wide closure rule. Glob paths are normative sets, not permission to skip files: G0 materializes every match at the implementation head, assigns it to one row/gate, and G5 proves zero unexplained old-success matches. Frozen predecessor package mirrors are evidence and are not edited.

| ID | Role | Path or pattern | Required change | Gate | Action |
|---|---|---|---|---|---|
| I001 | producer | crates/arcweft-adapter-context/src/manifest/nominal.rs | AdapterOpaqueTypeProducerId; declaration field/constructor/accessor | G2 | add/migrate |
| I002 | consumer | crates/arcweft-adapter-context/src/manifest.rs | public re-export; AdapterRustType delegated accessor; try_with_rust_manifest | G1/G2 | migrate |
| I003 | codec | crates/arcweft-adapter-context/src/codec.rs | schema 2 JSON/TOML preflight, private DTO, producer error mapping | G2 | replace |
| I004 | producer | crates/arcweft-adapter-context/src/standard.rs | native HTTP and inference tensor explicit constants/declarations | G2 | migrate |
| I005 | api-test | crates/arcweft-adapter-context/tests/public_symbol_api.rs | public producer newtype/accessor and no bypass surface | G2 | extend |
| I006 | producer | crates/arcweft-rust-abi/src/producer.rs | new Rust ABI producer ID/error owner | G1 | add |
| I007 | model | crates/arcweft-rust-abi/src/model.rs | schema version 2; required type field; builder/direct literals | G1 | replace |
| I008 | validation | crates/arcweft-rust-abi/src/validation.rs | schema/producer-first programmatic validation and typed errors | G1 | extend |
| I009 | display | crates/arcweft-rust-abi/src/display.rs | render producer in declaration diagnostics/display | G1 | extend |
| I010 | codec | crates/arcweft-rust-abi/src/lib.rs | exports and sole from_json/to_json entry points with preflight | G1 | replace |
| I011 | tests | crates/arcweft-rust-abi/src/tests.rs | all type literals/schema JSON/goldens gain producer | G1 | migrate |
| I012 | artifact | crates/arcweft-rust-abi-build/src/lib.rs | validated schema-2 pretty JSON and BLAKE3 artifact hash | G1 | migrate |
| I013 | macro | crates/arcweft-rust-abi-macros/src/lib.rs | helper attribute parse/validate/expand | G1 | extend |
| I014 | macro-pass | crates/arcweft-rust-abi-macros/tests/export.rs | PlayerScore/Rank/Pair explicit producers | G1 | migrate |
| I015 | macro-harness | crates/arcweft-rust-abi-macros/tests/compile_fail.rs | run new deterministic UI matrix | G1 | extend |
| I016 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_lifetime_generic_type.rs | add valid producer; retain lifetime error | G1 | migrate |
| I017 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_const_generic_type.rs | add valid producer; retain const error | G1 | migrate |
| I018 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_reference_field.rs | add valid producer; retain reference error | G1 | migrate |
| I019 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_generic_export.rs | function-only; no producer field | G1 | verify |
| I020 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_reference_export.rs | function-only; no producer field | G1 | verify |
| I021 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_reference_return.rs | function-only; no producer field | G1 | verify |
| I022 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_self_receiver_export.rs | function-only; no producer field | G1 | verify |
| I023 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_missing_opaque_producer.rs | missing attribute | G1 | add with .stderr |
| I024 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_duplicate_opaque_producer.rs | duplicate key | G1 | add with .stderr |
| I025 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_malformed_opaque_producer.rs | malformed helper | G1 | add with .stderr |
| I026 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_non_string_opaque_producer.rs | non-string value | G1 | add with .stderr |
| I027 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_unknown_arcweft_type_option.rs | unknown key | G1 | add with .stderr |
| I028 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_empty_opaque_producer.rs | empty value | G1 | add with .stderr |
| I029 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_control_opaque_producer.rs | control value | G1 | add with .stderr |
| I030 | macro-fail | crates/arcweft-rust-abi-macros/tests/ui/reject_reserved_opaque_producer.rs | reserved std. value | G1 | add with .stderr |
| I031 | registration | crates/arcweft-adapter-sema/src/registration.rs | sole conversion enum/errors; atomic source-backed facts | G3a | extend |
| I032 | registration-input | crates/arcweft-adapter-sema/src/registration/input.rs | mandatory producer on adapter/Rust rows | G3a | extend |
| I033 | digest | crates/arcweft-adapter-sema/src/registration/input/digest.rs | environment manifest v2 rows/domain; type-input v1 unchanged | G3a | replace/retain |
| I034 | source | crates/arcweft-adapter-sema/src/registration/input/source.rs | adapter-manifest-v2 rows and payload source map | G3a | replace |
| I035 | tests | crates/arcweft-adapter-sema/src/registration/tests.rs | projection/error/source/shared-domain/digest tests | G3a | extend |
| I036 | inventory | crates/arcweft-lang-sema/src/registration/environment_input.rs | mandatory RuntimeOpaqueTypeProducerId field/API | G3b | extend |
| I037 | registrar | crates/arcweft-lang-sema/src/registration/registrar.rs | call producer-bearing try_new_opaque | G3b | replace |
| I038 | catalog | crates/arcweft-lang-sema/src/env/nominal.rs | producer-bearing semantics/record/catalog digest v2 | G3b | replace |
| I039 | accepted-type | crates/arcweft-lang-sema/src/types/nominal.rs | AcceptedNominalType producer field/accessors/instantiation | G3b | extend |
| I040 | substitution | crates/arcweft-lang-sema/src/types/substitution.rs | clone producer through substituted arguments | G3b | replace |
| I041 | identity-digest | crates/arcweft-lang-sema/src/types/digest.rs | explicit declaration+arguments; producer excluded | G3b | audit/test |
| I042 | rust-metadata | crates/arcweft-lang-sema/src/registration/rust_metadata.rs | retain structural digest; consume declaration producer only via inventory | G3b | audit |
| I043 | loader | crates/arcweft-project-loader/** | all maintained adapter JSON/TOML and Rust ABI input/goldens | G4 | migrate |
| I044 | compiler | crates/arcweft-compiler/** | external environment fixtures and entry projection tests | G4 | migrate |
| I045 | lsp | crates/arcweft-lsp/** | manifest/Rust export fixtures and accepted environment snapshots | G4 | migrate |
| I046 | verify-lsp | crates/arcweft-verify-lsp/** | verification fixtures/goldens | G4 | migrate |
| I047 | desktop | crates/*desktop*/** | all ArcweftType derives and ArcweftRustTypeDecl literals | G4 | migrate |
| I048 | standard-adapters | crates/*adapter*/** | non-context standard/host manifests and Rust exports | G4 | migrate |
| I049 | examples | examples/** | maintained manifest/Rust derive examples | G4 | migrate |
| I050 | fixtures | tests/**; fixtures/**; testdata/** | all schema/generator/digest fixtures selected by closure search | G4 | migrate |
| I051 | docs | docs/** | maintained schema examples only; frozen package mirrors unchanged | G4 | migrate selectively |
| I052 | search-closure | AdapterNominalDeclaration::try_new | all call sites and direct struct literals | G0/G5 | enumerate then zero old-success matches |
| I053 | search-closure | ArcweftRustTypeDecl { | all direct Rust ABI type declaration literals | G0/G5 | enumerate then zero old-success matches |
| I054 | search-closure | #[derive(ArcweftType)] | all exported nominal derives | G0/G5 | enumerate then zero old-success matches |
| I055 | search-closure | schema_version: 1 / "schema_version": 1 / schema_version = 1 | all schema-1 writers/fixtures/goldens | G0/G5 | enumerate then zero old-success matches |
| I056 | search-closure | AcceptedNominalInventoryInput::new | all accepted inventory constructors | G0/G5 | enumerate then zero old-success matches |
| I057 | search-closure | AcceptedNominalSemantics::Opaque | all opaque catalog construction/patterns | G0/G5 | enumerate then zero old-success matches |
| I058 | search-closure | AcceptedNominalType::new | all instantiation/substitution constructors | G0/G5 | enumerate then zero old-success matches |
| I059 | search-closure | adapter-manifest-v1 | all generated-source fixtures/goldens | G0/G5 | enumerate then zero old-success matches |
| I060 | search-closure | arcweft.environment-manifest.v1 | all digest writers/vectors | G0/G5 | enumerate then zero old-success matches |
| I061 | search-closure | arcweft.accepted-nominal-catalog.v1 | all catalog digest writers/vectors | G0/G5 | enumerate then zero old-success matches |

## Exact closure commands

```text
git grep -n -E 'AdapterNominalDeclaration(::try_new)?|AdapterNominalDeclaration[[:space:]]*\{' -- crates examples tests fixtures testdata docs
git grep -n -E 'ArcweftRustTypeDecl[[:space:]]*\{|ArcweftRustTypeDecl::' -- crates examples tests fixtures testdata docs
git grep -n -E '#\[derive\([^]]*ArcweftType|derive\(ArcweftType' -- crates examples tests fixtures testdata docs
git grep -n -E 'schema_version[[:space:]]*[:=][[:space:]]*1|\"schema_version\"[[:space:]]*:[[:space:]]*1' -- . ':!docs/reviews/packages/**'
git grep -n -E 'AcceptedNominalInventoryInput|AcceptedNominalSemantics::Opaque|AcceptedNominalType::new|try_new_opaque' -- crates
git grep -n -E 'adapter-manifest-v1|arcweft\.environment-manifest\.v1|arcweft\.accepted-nominal-catalog\.v1' -- . ':!docs/reviews/packages/**'
git grep -n -E 'opaque_producer|opaque-producer|OpaqueTypeProducer' -- crates examples tests fixtures testdata docs
find . -type f \( -name '*.json' -o -name '*.toml' -o -name '*.stderr' -o -name '*.snap' -o -name '*.golden' \) -print0 | sort -z | xargs -0 grep -n -E 'schema_version|nominal_types|ArcweftType|adapter-manifest'
```

Every match is either migrated, deliberately unchanged with a reason recorded in the implementation note, or a frozen package/request mirror excluded from production migration. A new matching file discovered after this design return is automatically part of the same semantic inventory; this is not an open design decision.
