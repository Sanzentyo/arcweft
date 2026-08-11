# Test matrix

Total normative rows: **150**.

| ID | Area | Kind | Case | Expected | Gate |
| --- | --- | --- | --- | --- | --- |
| CORE-001 | core | positive | exact P/A accepts exact P/A owner | accept | A1.1 |
| CORE-002 | core | negative | exact P/A rejects exact P/B owner | reject | A1.1 |
| CORE-003 | core | negative | exact P/A rejects exact Q/A owner | reject | A1.1 |
| CORE-004 | core | positive | wide P/T accepts exact P/A owner | accept | A1.1 |
| CORE-005 | core | negative | wide P/T rejects exact Q/A owner | reject | A1.1 |
| CORE-006 | core | positive | wide P/T accepts identical wide P/T by equality | accept | A1.1 |
| CORE-007 | core | negative | wide P/T rejects wide P/U | reject | A1.1 |
| CORE-008 | core | positive | exact owner wraps validated payload | RuntimeValue::Opaque | A1.1 |
| CORE-009 | core | negative | producer-wide owner cannot wrap | NonConcreteOwner | A1.1 |
| CORE-010 | core | positive | exact checked type accepts exact opaque value | accept | A1.1 |
| CORE-011 | core | negative | exact checked type rejects different semantic identity | reject | A1.1 |
| CORE-012 | core | negative | exact checked type rejects different producer | reject | A1.1 |
| CORE-013 | core | positive | wide checked type accepts exact same-producer value | accept | A1.1 |
| CORE-014 | core | negative | opaque checked type rejects raw payload | reject | A1.1 |
| CORE-015 | core | negative | opaque checked type rejects raw NominalRecord | reject | A1.1 |
| CORE-016 | core | negative | no Dynamic native substitute | reject | A1.1 |
| CORE-017 | core | positive | opaque payload at nesting limit | accept | A1.1 |
| CORE-018 | core | negative | opaque payload beyond limit | typed nesting rejection | A1.1 |
| CORE-019 | core | positive | canonical opaque value begins with tag 16 | golden | A1.1 |
| CORE-020 | core | positive | canonical bytes include producer and exact identity | golden | A1.1 |
| CORE-021 | core | negative | runtime-only payload remains unencodable | existing encoding error | A1.1 |
| CORE-022 | core | positive | opaque checked/value carriers round trip | equal | A1.1 |
| CORE-023 | core | negative | missing owner field is rejected | deserialize error | A1.1 |
| CORE-024 | core | positive | parent nominal ID+layout acceptance remains exact | pass | A1.1 |
| CORE-025 | composite | positive | Ok of Result<opaque,opaque> retains both branches | one complete owner | A1.1/A1.2 |
| CORE-026 | composite | positive | Err of same Result retains identical complete owner | same type ID | A1.1/A1.2 |
| CORE-027 | composite | negative | missing error producer fails even when lowering Ok | projection error at ResultError | A1.1/A1.2 |
| CORE-028 | composite | negative | missing ok producer fails even when lowering Err | projection error at ResultOk | A1.1/A1.2 |
| CORE-029 | composite | positive | Result<Reduction<GameState>,AgentError> projects | closed complete owner | A1.1/A1.2 |
| CORE-030 | composite | positive | Result<Reduction<GameState>,ReducerError> projects | closed complete owner | A1.1/A1.2 |
| CORE-031 | composite | positive | Result<Unit,AgentError> projects | closed complete owner | A1.1/A1.2 |
| CORE-032 | composite | negative | Some opaque retains Option item | complete owner | A1.1/A1.2 |
| CORE-033 | composite | positive | None opaque retains same Option owner | same type ID | A1.1/A1.2 |
| CORE-034 | composite | positive | Option<Never>::None accepted | accept | A1.1/A1.2 |
| CORE-035 | composite | negative | Option<Never>::Some rejected | reject | A1.1/A1.2 |
| CORE-036 | composite | positive | Tuple([]) accepts empty tuple | accept | A1.1/A1.2 |
| CORE-037 | composite | negative | Tuple([]) rejects nonempty tuple | reject | A1.1/A1.2 |
| CORE-038 | composite | positive | tuple with two opaque producers validates positions | accept | A1.1/A1.2 |
| CORE-039 | composite | negative | first bad tuple projection path is index 0 | deterministic | A1.1/A1.2 |
| CORE-040 | composite | negative | Choice([]) accepts no value | reject | A1.1/A1.2 |
| CORE-041 | composite | positive | choice accepts first opaque alternative | accept | A1.1/A1.2 |
| CORE-042 | composite | positive | choice accepts second opaque alternative | accept | A1.1/A1.2 |
| CORE-043 | composite | negative | choice rejects unrelated producer | reject | A1.1/A1.2 |
| CORE-044 | composite | positive | empty opaque sequence accepted | accept | A1.1/A1.2 |
| CORE-045 | composite | positive | all elements same exact opaque type | accept | A1.1/A1.2 |
| CORE-046 | composite | negative | one mismatched element rejects sequence | reject | A1.1/A1.2 |
| CORE-047 | composite | positive | Reduction<Result<GameState,AgentError>> is atomic opaque leaf | no recursive schema demand | A1.1/A1.2 |
| CORE-048 | composite | positive | recursive semantic graph crossing opaque producer terminates | success | A1.1/A1.2 |
| CORE-049 | composite | negative | structural recursion without opaque cut | typed recursion rejection | A1.1/A1.2 |
| CORE-050 | composite | negative | Never accepts no runtime value | reject | A1.1/A1.2 |
| CORE-051 | composite | positive | selected Ok never rewrites E to Never | full owner | A1.1/A1.2 |
| CORE-052 | composite | positive | distinct exact CharacterDialogue joins producer-wide Any | wide owner | A1.1/A1.2 |
| PROD-001 | producer/projection | negative | producerless accepted Opaque cannot compile/construct | constructor unavailable | A1.2 |
| PROD-002 | producer/projection | positive | instantiated AcceptedNominalType retains exact producer | equal | A1.2 |
| PROD-003 | producer/projection | negative | TypeKind::Named direct runtime projection | MissingOpaqueProducerEvidence | A1.2 |
| PROD-004 | producer/projection | negative | Named is not hashed into layout or producer | no success path | A1.2 |
| PROD-005 | producer/projection | negative | accepted Rust metadata cannot synthesize layout | typed catalog failure | A1.2 |
| PROD-006 | producer/projection | positive | Reduction<T> uses std.reduction | equal | A1.2 |
| PROD-007 | producer/projection | positive | ArcError uses std.arc_error | equal | A1.2 |
| PROD-008 | producer/projection | positive | ReducerError uses std.reducer_error | equal | A1.2 |
| PROD-009 | producer/projection | positive | AgentError uses std.agent_error | equal | A1.2 |
| PROD-010 | producer/projection | positive | CharacterDialogue uses canonical dialogue producer | equal | A1.2 |
| PROD-011 | producer/projection | positive | exact dialogue type projects ExactIdentity | equal | A1.2 |
| PROD-012 | producer/projection | positive | any dialogue type projects ProducerWide | equal | A1.2 |
| PROD-013 | producer/projection | positive | validated exact dialogue wraps opaque record payload | success | A1.2 |
| PROD-014 | producer/projection | negative | Any owner cannot create concrete dialogue value | NonConcreteOwner | A1.2 |
| PROD-015 | producer/projection | negative | dialogue decoder rejects foreign producer | typed error | A1.2 |
| PROD-016 | producer/projection | negative | dialogue decoder retains existing schema validation | typed error | A1.2 |
| PROD-017 | producer/projection | negative | decoded character semantic identity must match wrapper | typed error | A1.2 |
| PROD-018 | producer/projection | positive | project nominal still uses schema.try_layout_hash | exact layout | A1.2 |
| PROD-019 | producer/projection | negative | schema digest direct TypeLayoutHash conversion absent | no fallback | A1.2 |
| PROD-020 | producer/projection | positive | RuntimeTypeShape::Named no longer exists | compile-clean | A1.2 |
| PROD-021 | producer/projection | positive | opaque shape requires producer/admission | compile-clean | A1.2 |
| PROD-022 | producer/projection | positive | Result projects ok before error | deterministic | A1.2 |
| PROD-023 | producer/projection | positive | tuple projects increasing index | deterministic | A1.2 |
| PROD-024 | producer/projection | positive | choice projects increasing index | deterministic | A1.2 |
| PROD-025 | producer/projection | positive | required reducer entry signature closes | success | A1.2 |
| PROD-026 | producer/projection | positive | required agent entry signature closes | success | A1.2 |
| VAR-001 | variant | positive | checked_selection returns full Result owner + Ok case | success | A1.2 |
| VAR-002 | variant | positive | checked_selection returns same owner + Err case | success | A1.2 |
| VAR-003 | variant | positive | checked_selection returns full Option owner | success | A1.2 |
| VAR-004 | variant | positive | None returns canonical payloadless case | success | A1.2 |
| VAR-005 | variant | positive | nominal enum case selected by source ordinal | success | A1.2 |
| VAR-006 | variant | negative | out-of-range ordinal | CaseOrdinal | A1.2 |
| VAR-007 | variant | negative | resolved name differs from canonical case | CaseName | A1.2 |
| VAR-008 | variant | negative | payload required but absent | typed lowering error | A1.2 |
| VAR-009 | variant | negative | payload absent case receives payload | typed lowering error | A1.2 |
| VAR-010 | variant | positive | pattern retains full owner | equal | A1.2 |
| VAR-011 | variant | positive | scrutinee and pattern complete owner agree | accept | A1.2 |
| VAR-012 | variant | positive | selected-case Never helper removed | compile-clean | A1.2 |
| VAR-013 | variant | positive | accepts_variant_case removed | compile-clean | A1.2 |
| VAR-014 | variant | positive | direct public owner projection removed | compile-clean | A1.2 |
| AWBC-001 | AWBC | positive | codec header is 11 and ABI is 1 | golden | A1.3 |
| AWBC-002 | AWBC | positive | opaque runtime type tag 23 bytes | golden | A1.3 |
| AWBC-003 | AWBC | positive | admission 0 decodes exact | equal | A1.3 |
| AWBC-004 | AWBC | positive | admission 1 decodes wide | equal | A1.3 |
| AWBC-005 | AWBC | negative | unknown admission tag | codec error | A1.3 |
| AWBC-006 | AWBC | negative | invalid producer string index | structural error | A1.3 |
| AWBC-007 | AWBC | negative | invalid producer ID spelling | structural error | A1.3 |
| AWBC-008 | AWBC | positive | opaque constant tag 18 bytes | golden | A1.3 |
| AWBC-009 | AWBC | positive | exact opaque constant materializes | RuntimeValue::Opaque | A1.3 |
| AWBC-010 | AWBC | negative | producer-wide opaque constant | structural error | A1.3 |
| AWBC-011 | AWBC | negative | constant type row is not opaque | structural error | A1.3 |
| AWBC-012 | AWBC | negative | missing payload constant | structural error | A1.3 |
| AWBC-013 | AWBC | negative | cyclic opaque payload constant | structural error | A1.3 |
| AWBC-014 | AWBC | positive | equal opaque owners intern one type ID | equal | A1.3 |
| AWBC-015 | AWBC | positive | different exact identities intern distinct IDs | distinct | A1.3 |
| AWBC-016 | AWBC | positive | wide row distinct from exact row | distinct | A1.3 |
| AWBC-017 | AWBC | positive | Ok and Err use one complete Result type ID | equal | A1.3 |
| AWBC-018 | AWBC | positive | MakeVariant Ok payload type validates | pass | A1.3 |
| AWBC-019 | AWBC | positive | MakeVariant Err opaque payload validates | pass | A1.3 |
| AWBC-020 | AWBC | negative | wrong opaque producer payload | verify error | A1.3 |
| AWBC-021 | AWBC | positive | AWBC variant pattern complete owner agrees | pass | A1.3 |
| AWBC-022 | AWBC | negative | pattern owner selected-case mismatch | verify error | A1.3 |
| AWBC-023 | AWBC | positive | equal exact opaque branch rows merge | pass | A1.3 |
| AWBC-024 | AWBC | positive | exact dialogue branches merge to checker-emitted wide row | pass | A1.3 |
| AWBC-025 | AWBC | negative | verifier does not invent producer-wide join | verify error | A1.3 |
| AWBC-026 | AWBC | positive | exact arg to exact param | pass | A1.3 |
| AWBC-027 | AWBC | positive | exact dialogue arg to wide param same producer | pass | A1.3 |
| AWBC-028 | AWBC | negative | exact foreign producer to wide param | verify error | A1.3 |
| AWBC-029 | AWBC | positive | opaque return exact row | pass | A1.3 |
| AWBC-030 | AWBC | negative | opaque return wrong identity | verify error | A1.3 |
| AWBC-031 | AWBC | positive | VM and native accept exact value identically | equal | A1.3 |
| AWBC-032 | AWBC | positive | VM and native accept wide expected identically | equal | A1.3 |
| AWBC-033 | AWBC | negative | codec-10 bytes at codec-11 reader | version error | A1.3 |
| AWBC-034 | AWBC | negative | unknown runtime type tag | codec error | A1.3 |
| SAVE-001 | persistence | positive | session save schema is 3 | golden | A1.4 |
| SAVE-002 | persistence | positive | opaque register/fiber/snapshot round trip | equal | A1.4 |
| SAVE-003 | persistence | positive | nested opaque composite round trip | equal | A1.4 |
| SAVE-004 | persistence | negative | tampered semantic identity rejected by slot/producer | atomic reject | A1.4 |
| SAVE-005 | persistence | negative | tampered producer rejected | atomic reject | A1.4 |
| SAVE-006 | persistence | negative | producer decode failure aborts restore | atomic reject | A1.4 |
| SAVE-007 | persistence | negative | too-deep opaque payload aborts restore | atomic reject | A1.4 |
| SAVE-008 | persistence | negative | schema-2 save rejected before value decode | version error | A1.4 |
| SAVE-009 | persistence | positive | outer awbc_v1 key remains | equal | A1.4 |
| SAVE-010 | persistence | positive | codec-11 bytes change exact bundle digest | changed | A1.4 |
| SAVE-011 | persistence | positive | exact-byte cache invalidates | miss | A1.4 |
| SAVE-012 | persistence | positive | only schema-3/codec-11 readers remain | compile-clean | A1.4 |
| SAVE-013 | persistence | positive | new public Serde shapes pinned | golden | A1.4 |
| SAVE-014 | persistence | positive | RuntimeTypeSchema golden bytes unchanged | equal | A1.4 |
| GATE-001 | gate | verification | format after A1.1 | pass | named gate |
| GATE-002 | gate | verification | workspace check after A1.1 | pass | named gate |
| GATE-003 | gate | verification | workspace Clippy after A1.1 | pass | named gate |
| GATE-004 | gate | verification | workspace check/Clippy after A1.2 | pass | named gate |
| GATE-005 | gate | verification | workspace check/Clippy after A1.3 | pass | named gate |
| GATE-006 | gate | verification | workspace check/Clippy after A1.4 | pass | named gate |
| GATE-007 | gate | verification | current compiler entry suite | pass | named gate |
| GATE-008 | gate | verification | core focused suite | pass | named gate |
| GATE-009 | gate | verification | runtime-plan focused suite | pass | named gate |
| GATE-010 | gate | verification | final structure audit has no deleted/fallback path | pass | named gate |
