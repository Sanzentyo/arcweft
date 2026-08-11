# Producer, consumer, and deletion inventory

| Symbol/surface | Action | Owner | Final responsibility | Evidence/target | Gate |
| --- | --- | --- | --- | --- | --- |
| core.pattern.RuntimeCheckedType | change | arcweft-core | add Opaque; parent Nominal layout; inherent accepts_value/variant_case | pattern.rs | A1.1 |
| core.pattern.RuntimeOpaqueTypeProducerId | add | arcweft-core | validated typed producer ID | pattern.rs | A1.1 |
| core.pattern.RuntimeOpaqueTypeAdmission | add | arcweft-core | ExactIdentity/ProducerWide closed policy | pattern.rs | A1.1 |
| core.pattern.RuntimeOpaqueTypeOwner | add | arcweft-core | single checked opaque owner and compatibility | pattern.rs | A1.1 |
| core.value.RuntimeOpaqueValue | add | arcweft-core | exact producer/semantic evidence + payload | value.rs | A1.1 |
| core.value.RuntimeValue | change | arcweft-core | add Opaque and exhaustive traversal | value.rs + nesting/canonical modules | A1.1 |
| runtime_value_matches_pattern_type | delete | arcweft-core | replaced by RuntimeCheckedType::accepts_value | pattern.rs | A1.1 |
| RuntimeCheckedType::accepts_variant_case | delete | arcweft-core | replaced by variant_case + checked selection | pattern.rs | A1.1 |
| RuntimeTypeSchema | preserve | arcweft-core | no opaque schema and no hash changes | entry schema | A1.1 |
| RuntimeNominalRecordLayout | preserve | arcweft-core | parent exact layout owner | value nominal_record | A1.1 |
| AcceptedNominalSemantics::Opaque | change | arcweft-lang-sema | mandatory producer field | env/nominal.rs | A1.2 |
| AcceptedNominalRecord::try_new_opaque | add | arcweft-lang-sema | sole opaque accepted record constructor | env/nominal.rs | A1.2 |
| AcceptedNominalType | change | arcweft-lang-sema | retain mandatory producer | types/nominal.rs | A1.2 |
| standard accepted domain atoms | change | arcweft-lang-sema | Named success to producer-bearing opaque accepted rows | env/nominal.rs | A1.2 |
| accepted Rust metadata | change | arcweft-lang-sema | consume declared producer; never derive schema/layout | env/rust_metadata.rs | A1.2 |
| TypeKind::Named | preserve/fail | arcweft-lang-sema/compiler | compile-time only; runtime projection typed failure | types.rs + compiler projection | A1.2 |
| TypeKind::AcceptedNominal | change | arcweft-lang-sema/compiler | runtime opaque projection from retained producer | types.rs | A1.2 |
| TypeKind::CharacterDialogue | change | arcweft-lang-sema/compiler | exact/producer-wide opaque projection | types/character_dialogue.rs | A1.2 |
| CharacterDialogueRuntimeSchema | change | arcweft-dialogue | inherent producer and opaque decode validation | character_dialogue/schema.rs | A1.2 |
| CharacterDialogueValue::into_runtime_value | replace | arcweft-dialogue | try_into_runtime_value with exact owner wrapper | character_dialogue/schema.rs | A1.2 |
| RuntimeTypeShape::Named | delete | arcweft-runtime-plan | no runtime name/layout fallback | semantic_facts.rs | A1.2 |
| RuntimeTypeShape::Opaque | change | arcweft-runtime-plan | required producer+admission | semantic_facts.rs | A1.2 |
| RuntimeNormalizedType::checked_type | change | arcweft-runtime-plan | typed error/path and recursive opaque projection | semantic_facts.rs | A1.2 |
| RuntimeCheckedTypeProjectionError | add | arcweft-runtime-plan | closed typed projection errors | semantic_facts.rs | A1.2 |
| RuntimeResolvedVariant::checked_selection | add | arcweft-runtime-plan | single complete owner/case projection | semantic_facts.rs | A1.2 |
| RuntimeVariantOwner::checked_type | delete/public-close | arcweft-runtime-plan | private projection only through selection | semantic_facts.rs | A1.2 |
| final expr variant lowering | change | arcweft-runtime-plan | consume checked selection | final expr lowering | A1.2 |
| final pattern variant lowering | change | arcweft-runtime-plan | consume checked selection | final pattern lowering | A1.2 |
| compiler nominal schema digest fallback | delete | arcweft-compiler | use schema.try_layout_hash and parity | runtime semantic projection | A1.2 |
| entry role Reduction/ReducerError | change | arcweft-compiler/runtime-plan | complete Result owner | entry-role lowering | A1.2 |
| entry role Unit/AgentError | change | arcweft-compiler/runtime-plan | complete Result owner | entry-role lowering | A1.2 |
| AwbcRuntimeType | change | arcweft-core AWBC | Opaque tag 23 | awbc/schema.rs + codec/types.rs | A1.3 |
| AwbcConstant | change | arcweft-core AWBC | Opaque tag 18 exact row + payload | awbc/schema.rs + codec/types.rs | A1.3 |
| AWBC_CODEC_VERSION | change | arcweft-core AWBC | 10 -> 11 | awbc/schema.rs | A1.3 |
| AWBC_ABI_VERSION | preserve | arcweft-core AWBC | remains 1 | awbc/schema.rs | A1.3 |
| AWBC structural verifier | change | arcweft-core AWBC | opaque row/constant/MakeVariant/pattern checks | awbc/verify/structure.rs | A1.3 |
| AWBC fiber/VM type matcher | change | arcweft-core AWBC | core owner/value acceptance parity | awbc/fiber.rs | A1.3 |
| AWBC expr/pattern lowerers | change | arcweft-runtime-plan | complete selection and opaque interning | awbc_lower/*.rs | A1.3 |
| AWBC types_compatible | change | arcweft-core AWBC | exact/wide relation; no variant covariance | awbc verifier/fiber | A1.3 |
| canonical runtime value codec | change | arcweft-core | tag 16 and payload traversal | value canonical encoding | A1.1/A1.4 |
| session save schema | change | arcweft-runtime-driver/save | 2 -> 3; hard old rejection | session_save | A1.4 |
| bundle AWBC product key | preserve | arcweft-bundle | awbc_v1 remains; inner codec 11 | bundle product codec | A1.4 |
| snapshot/fiber/capture values | change | runtime/save consumers | Serde RuntimeValue::Opaque | fiber/snapshot consumers | A1.4 |
| codec-10 reader/goldens | delete | arcweft-core AWBC | single codec-11 reader | codec/tests | A1.4 |
| save-schema-2 reader | delete | runtime-driver/save | single schema-3 reader | save/tests | A1.4 |
| name/digest/layout fallback | delete | compiler/runtime-plan | typed failure only | all projection consumers | A1.2/A1.4 |
| producer registry/trait | prohibit | all | no side table/optional predicate/extension trait | n/a | all |

## Closure rule

Every `change` or `add` row is accompanied by focused tests in `TEST_MATRIX.csv`. Every `delete` row is removed in the named compile-clean gate without alias/deprecation/dual path. `preserve` rows are regression-tested and are not redesigned.
