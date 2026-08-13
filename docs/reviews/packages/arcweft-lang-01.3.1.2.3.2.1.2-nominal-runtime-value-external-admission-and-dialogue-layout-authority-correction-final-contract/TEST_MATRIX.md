# Normative test matrix

Rows: 80. Focused rows are not replaced by full-gate rows. Commands are implementation requirements, not claimed results in this design-only archive.

| ID | Area | Gate | Kind | Setup | Action | Expected | Owner |
|---|---|---|---|---|---|---|---|
| NREA-001 | catalog | G2 | positive | one canonical layout and project reachability | admit catalog | one operational entry; structural descriptor retained | core unit |
| NREA-002 | catalog | G2 | negative | same key twice equal | admit | DuplicateLayout before producer rows | core unit |
| NREA-003 | catalog | G2 | negative | same key with different field predicate | admit | ConflictingLayout | core unit |
| NREA-004 | catalog | G2 | negative | descriptor scalars differ from key | admit | LayoutKeyMismatch | core unit |
| NREA-005 | catalog | G2 | negative | producer key missing globally | admit | MissingProducerLayout | core unit |
| NREA-006 | catalog | G2 | negative | unreachable global layout | admit | UnreachableLayout | core/plan unit |
| NREA-007 | catalog | G2 | negative | duplicate producer/key | admit | DuplicateProducerRecord | core unit |
| NREA-008 | handle | G2 | positive | admitted external producer + layout [a,z] | require and construct [va,vz] | checked nominal value published only through handle | core unit |
| NREA-009 | handle | G2 | negative | wrong field count | try_construct | RuntimeNominalRecordError::FieldCount | core unit |
| NREA-010 | handle | G2 | negative | wrong first field type | try_construct | FieldType for ID 1 before later fields | core unit |
| NREA-011 | handle | G2 | compile_fail | external caller constructs RuntimeNominalRecordAdmission literal | compile | private fields reject | trybuild |
| NREA-012 | handle | G2 | compile_fail | external caller calls handle constructor/default/deserializes | compile | surface absent | trybuild |
| NREA-013 | handle | G8 | compile_fail | arbitrary nominal/layout/fields call RuntimeNominalRecordValue::new | compile | method absent | trybuild |
| NREA-014 | handle | G8 | compile_fail | obsolete internal call site uses new | compile internal fixture | method absent | core compile-fail |
| NREA-015 | handle | G8 | compile_fail | call validate_shape | compile | method absent | trybuild |
| NREA-016 | lookup | G2 | negative | producer absent | producer() | ProducerNotAdmitted | core unit |
| NREA-017 | lookup | G2 | negative | nominal absent | require | Missing | core unit |
| NREA-018 | lookup | G2 | negative | nominal present but semantic differs | require | StaleSemanticIdentity | core unit |
| NREA-019 | lookup | G2 | negative | nominal+semantic present but layout differs | require | StaleLayout | core unit |
| NREA-020 | lookup | G2 | negative | exact key authorized only to other producer | require | WrongProducer with admitted producers | core unit |
| NREA-021 | lookup | G2 | precedence | producer absent and nominal absent | require | ProducerNotAdmitted | core unit |
| NREA-022 | lookup | G2 | precedence | stale semantic and wrong producer | require | StaleSemanticIdentity before WrongProducer | core unit |
| NREA-023 | tree | G2 | positive | nested nominal in tuple/sequence | validate | all descriptors and fields accepted with exact paths | core unit |
| NREA-024 | tree | G2 | negative | nested value wrong nominal+layout | validate | Type before Layout | core unit |
| NREA-025 | tree | G2 | negative | nested correct identity wrong count/type | validate | FieldCount before FieldType | core unit |
| NREA-026 | tree | G2 | negative | first of two fields wrong | validate | first defining-order FieldType path | core unit |
| NREA-027 | variant | G1 | positive | exact nominal variant case/payload | accepts_value | true | core unit |
| NREA-028 | variant | G1 | negative | right owner wrong ordinal/name | accepts_value | false | core unit |
| NREA-029 | variant | G1 | negative | right case wrong payload presence/type | accepts_value | false | core unit |
| NREA-030 | plan | G3 | positive | verified raw plan with canonical catalog | try_admit | AdmittedRuntimePlan and catalog | core plan unit |
| NREA-031 | plan | G3 | negative | catalog conflict plus executable plan | try_admit | RuntimePlanError::NominalRecordCatalog; no wrapper | core plan unit |
| NREA-032 | plan | G3 | structural | runtime constructor accepts raw RuntimePlan | compile/audit | signature absent; admitted wrapper required | structure test |
| NREA-033 | dialogue root | G5 | positive | valid CharacterDialogue domain value | try_encode | exact std.character_dialogue opaque tuple18 | dialogue unit |
| NREA-034 | dialogue root | G5 | positive | valid exact opaque tuple18 | try_decode_opaque | same domain value and canonical bytes | dialogue unit |
| NREA-035 | dialogue root | G5 | negative | wrong opaque producer | decode | OpaqueProducer before payload | dialogue unit |
| NREA-036 | dialogue root | G5 | negative | payload not tuple or count !=18 | decode | PayloadShape RootTuple18 | dialogue unit |
| NREA-037 | dialogue root | G5 | negative | exact producer but semantic ID for another character | decode | OpaqueSemanticIdentity before nested fields | dialogue unit |
| NREA-038 | dialogue root | G5 | negative | nested stage nominal missing descriptor | decode | RoleValue(Stage)->Lookup Missing | dialogue unit |
| NREA-039 | dialogue root | G5 | negative | nested style stale layout | decode | RoleValue(Style)->Lookup StaleLayout | dialogue unit |
| NREA-040 | dialogue root | G5 | negative | nested value authorized to other producer | decode | RoleValue->WrongProducer | dialogue unit |
| NREA-041 | dialogue root | G5 | precedence | nested wrong type and stale descriptor | decode | descriptor error before value type | dialogue unit |
| NREA-042 | custom | G5 | positive | sorted tuple2 entries with closed descriptors | encode/decode | round trip; no declared nominal/layout bytes | dialogue unit |
| NREA-043 | custom | G5 | negative | unknown custom ID | decode | UnknownCustomField | dialogue unit |
| NREA-044 | custom | G5 | negative | duplicate/out-of-order IDs | decode | NonCanonicalCustomOrder; no fallback | dialogue unit |
| NREA-045 | custom | G5 | negative | tuple arity 4 legacy entry | decode | PayloadShape CustomEntryTuple2 | dialogue unit |
| NREA-046 | custom | G5 | structural | custom schema source | audit | no Named("Dynamic"), custom nominal ID/layout | structure test |
| NREA-047 | inline | G5 | positive | every InlineFailure/InlineFallback/FallbackStyle case | encode/decode | exact direct variant round trip | dialogue unit |
| NREA-048 | inline | G5 | negative | wrong owner/ordinal/name/payload | decode | typed payload shape/case rejection | dialogue unit |
| NREA-049 | inline | G5 | structural | legacy std.inline_failure_policy nominal wrapper | audit | absent | structure test |
| NREA-050 | typed value | G6 | compile_fail | deserialize live CharacterDialogueTypedValue | compile | Deserialize impl absent | trybuild |
| NREA-051 | typed value | G6 | compile_fail | call raw try_new(nominal,layout,value) | compile | constructor absent | trybuild |
| NREA-052 | typed value | G6 | positive | schema admits each role and custom value | admit | normalized/validated live wrapper | dialogue unit |
| NREA-053 | normalize | G6 | positive | nested nominal with finite -0 floats | normalize | +0 and checked handle rebuild preserving order | dialogue unit |
| NREA-054 | normalize | G6 | negative | nested nominal without active descriptor | normalize | Lookup error; no reconstructed value | dialogue unit |
| NREA-055 | normalize | G6 | negative | non-finite nested float | normalize | typed error at exact path; no publish | dialogue unit |
| NREA-056 | clear | G6 | positive | Option/Sequence/Unit | clear | None/empty/Unit | dialogue unit |
| NREA-057 | clear | G6 | positive | nominal whose all fields are emptiable | clear | handle rebuild and validate | dialogue unit |
| NREA-058 | clear | G6 | negative | primitive/opaque/nonclearable custom | clear | typed rejection; original unchanged | dialogue unit |
| NREA-059 | clear | G6 | structural | anonymous record fallback | audit | Record(Vec::new()) fallback absent | structure test |
| NREA-060 | patch | G6 | positive | tuple/sequence/nominal/variant paths | preflight+apply | correct RuntimeValuePath semantics and validated result | dialogue unit |
| NREA-061 | patch | G6 | negative | duplicate/prefix-overlap paths | preflight | OverlappingStructuredPaths before mutation | dialogue unit |
| NREA-062 | patch | G6 | negative | later path missing after earlier valid path | preflight | PatchPath; original byte-identical | dialogue unit |
| NREA-063 | patch | G6 | negative | replacement wrong nested nominal layout | preflight | PatchValue Lookup/Nominal error; no mutation | dialogue unit |
| NREA-064 | patch | G6 | negative | late domain validation fails | apply candidate | no partial publication | dialogue integration |
| NREA-065 | patch | G8 | compile_fail | construct dialogue RuntimeFieldPath | compile | type absent | trybuild |
| NREA-066 | restore | G7 | negative | saved nominal type/layout/count malformed | restore | typed precedence before ownership traversal | driver/save integration |
| NREA-067 | restore | G7 | negative | saved nested descriptor missing/stale/wrong producer | restore | typed lookup error with RuntimeValuePath | driver/save integration |
| NREA-068 | restore | G7 | positive | valid CharacterDialogue opaque save | restore | schema decode before session activation | driver integration |
| NREA-069 | replay | G7 | negative | replay value malformed nominal | replay | typed failure before transition application | root/replay integration |
| NREA-070 | View | G7 | negative | View input malformed CharacterDialogue | mount/restore | CharacterDialogue source before mount | driver/View integration |
| NREA-071 | bundle | G7 | negative | bundle plan catalog conflict | start session | DecodeBytecode(RuntimePlanError::NominalRecordCatalog) | bundle/driver integration |
| NREA-072 | canonical | G10 | golden | ordinary anonymous vs nominal records | encode | existing distinct bytes unchanged | core golden |
| NREA-073 | canonical | G10 | golden | CharacterDialogue exact opaque tuple | encode | new fixed bytes; no old nominal wrapper | dialogue golden |
| NREA-074 | canonical | G10 | golden | custom tuple2 and direct inline variant | encode | one representation each; no fallback | dialogue golden |
| NREA-075 | A1-A3 | G9 | regression | all retained parent identity/order tests | run | green unchanged | workspace |
| NREA-076 | versions | G9/G10 | structural | all touched version constants/manifests | audit | every Arcweft-owned version exactly 1 | structure test |
| NREA-077 | workspace | G9 | full_gate | final A4 tree | fmt/check/test/clippy | green | workspace |
| NREA-078 | structure | G9 | full_gate | final A4 tree | just structure-audit | no fallback/dual reader/copied table | workspace |
| NREA-079 | Tier 2 | G9 | full_gate | applicable exact-commit policy commands | run | green | workspace |
| NREA-080 | A6 | G10 | full_gate | codec/golden/tamper cross-product | run | green with versions 1 | workspace |
