# Complete direct test matrix

This matrix is normative. Each row names a directly observable behavior, type,
codec, compile-fail, or command result. Source text searches are not evidence.

Legend:

- `focused`: changed-crate/unit/integration loop;
- `workspace`: normal push-cut workspace gate;
- `tier2`: slow Agent/MCP/browser/render/save/replay validation;
- `audit`: structured dependency/size/ownership audit.


| Category | ID | Owner | Kind | Case | Required assertion | Gate |
|---|---|---|---|---|---|---|
| Syntax | `SYN-001` | `arcweft-lang-syntax` | unit | `alice[content]` | one DialogueContentApplication Bracket node; target path preserved | focused |
| Syntax | `SYN-002` | `arcweft-lang-syntax` | unit | `alice: content` | one DialogueContentApplication Colon node; no SpeakerLine | focused |
| Syntax | `SYN-003` | `arcweft-lang-syntax` | unit | `alice(look=smile)[content]` | target is ordinary Call with exact ArgumentListSyntax | focused |
| Syntax | `SYN-004` | `arcweft-lang-syntax` | unit | `configured(look=worried): content` | colon target keeps full call expression/ranges | focused |
| Syntax | `SYN-005` | `arcweft-lang-syntax` | unit | missing `)` | recovered ordinary call with exact close insertion | focused |
| Syntax | `SYN-006` | `arcweft-lang-syntax` | unit | missing `]` | recovered content application with exact insertion | focused |
| Syntax | `SYN-007` | `arcweft-lang-syntax` | unit | empty `[]` | empty DialogueContent node retained | focused |
| Syntax | `SYN-008` | `arcweft-lang-syntax` | unit | colon without body | empty content range plus ordinary missing-content diagnostic | focused |
| Syntax | `SYN-009` | `arcweft-lang-syntax` | unit | aligned `with:` | plan attached to application | focused |
| Syntax | `SYN-010` | `arcweft-lang-syntax` | unit | aligned `with {}` | same plan AST as indentation form | focused |
| Syntax | `SYN-011` | `arcweft-lang-syntax` | unit | misaligned `with:` | ordinary indentation diagnostic; no stolen block | focused |
| Syntax | `SYN-012` | `arcweft-lang-syntax` | unit | `items[0]` | index candidate, not dialogue application | focused |
| Syntax | `SYN-013` | `arcweft-lang-syntax` | unit | `[a,b]` | collection literal unchanged | focused |
| Syntax | `SYN-014` | `arcweft-lang-syntax` | unit | record literal in call | ordinary positional call argument preserved | focused |
| Syntax | `SYN-015` | `arcweft-lang-syntax` | unit | dialogue controls in bracket | dialogue candidate retained | focused |
| Syntax | `SYN-016` | `arcweft-lang-syntax` | unit | `.say(...)` | ordinary selected call only; no special AST/diagnostic | focused |
| Syntax | `SYN-017` | `arcweft-lang-syntax` | unit | Unicode/CRLF colon | all ranges UTF-8-boundary correct | focused |
| Syntax | `SYN-018` | `arcweft-lang-syntax` | unit | comments around head/content | comments/ranges preserved | focused |
| Syntax | `SYN-019` | `arcweft-lang-syntax` | unit | nested `foo()[content]` | complete nested target expression retained | focused |
| Syntax | `SYN-020` | `arcweft-lang-syntax` | unit | bare block after content | not attached unless `with` keyword present | focused |
| HIR and IDs | `HIR-001` | `arcweft-lang-hir` | unit | bracket lowering | Hir target is AuthoredExpr; no callee String | focused |
| HIR and IDs | `HIR-002` | `arcweft-lang-hir` | unit | colon lowering | same HirDialogueContentApplication meaning | focused |
| HIR and IDs | `HIR-003` | `arcweft-lang-hir` | unit | nested configuration call | argument source identity survives | focused |
| HIR and IDs | `HIR-004` | `arcweft-lang-hir` | unit | flow generated ID #1 | `say.flow.<full-flow>.001` | focused |
| HIR and IDs | `HIR-005` | `arcweft-lang-hir` | unit | flow named scopes | scope segments included in source-site ID | focused |
| HIR and IDs | `HIR-006` | `arcweft-lang-hir` | unit | callable generated ID | package/module/owner/name source-site prefix | focused |
| HIR and IDs | `HIR-007` | `arcweft-lang-hir` | unit | generated ordinal 1000 | minimum three digits, no 999 cap | focused |
| HIR and IDs | `HIR-008` | `arcweft-lang-hir` | unit | relative `@.x` | source-owner prefix + x | focused |
| HIR and IDs | `HIR-009` | `arcweft-lang-hir` | unit | family relative `@say:.x` | same normalized line ID | focused |
| HIR and IDs | `HIR-010` | `arcweft-lang-hir` | unit | relative parent scope | walks named scopes only | focused |
| HIR and IDs | `HIR-011` | `arcweft-lang-hir` | unit | relative walks above owner | source-ranged error | focused |
| HIR and IDs | `HIR-012` | `arcweft-lang-hir` | unit | absolute wrong family | AW-CD-013 | focused |
| HIR and IDs | `HIR-013` | `arcweft-lang-hir` | unit | duplicate explicit IDs | AW-CD-020 with both spans | focused |
| HIR and IDs | `HIR-014` | `arcweft-lang-hir` | unit | generated collides explicit | hard collision; no ordinal skipping | focused |
| HIR and IDs | `HIR-015` | `arcweft-lang-hir` | unit | dynamic character branch | generated ID independent of CharacterId | focused |
| HIR and IDs | `HIR-016` | `arcweft-lang-hir` | unit | character rename | generated source-site ID unchanged | focused |
| HIR and IDs | `HIR-017` | `arcweft-lang-hir` | unit | absent text key | derive text.* from complete say body | focused |
| HIR and IDs | `HIR-018` | `arcweft-lang-hir` | unit | explicit text key wrong family | source-ranged family error | focused |
| HIR and IDs | `HIR-019` | `arcweft-lang-hir` | unit | ownerless application no absolute ID | structured missing-owner/ID error | focused |
| HIR and IDs | `HIR-020` | `arcweft-lang-hir` | unit | source map revision | exact accepted document identity retained | focused |
| Sema and resolver | `SEM-001` | `arcweft-lang-sema` | unit | exact Character factory | result CharacterDialogue<Exact(C)> | focused |
| Sema and resolver | `SEM-002` | `arcweft-lang-sema` | unit | dynamic Ref<Character> factory | result CharacterDialogue<Any> | focused |
| Sema and resolver | `SEM-003` | `arcweft-lang-sema` | unit | empty call | complete CharacterDialogue, no partial function | focused |
| Sema and resolver | `SEM-004` | `arcweft-lang-sema` | unit | same-value reconfigure | same exact payload | focused |
| Sema and resolver | `SEM-005` | `arcweft-lang-sema` | unit | direct bracket target Character | implicit empty factory + application | focused |
| Sema and resolver | `SEM-006` | `arcweft-lang-sema` | unit | colon Character | same checked line as bracket | focused |
| Sema and resolver | `SEM-007` | `arcweft-lang-sema` | unit | exact look shorthand | expected exact manifest look family | focused |
| Sema and resolver | `SEM-008` | `arcweft-lang-sema` | unit | Any look shorthand | rejected as ambiguous owner | focused |
| Sema and resolver | `SEM-009` | `arcweft-lang-sema` | unit | Any fully typed look | accepted with runtime owner check fact | focused |
| Sema and resolver | `SEM-010` | `arcweft-lang-sema` | unit | same exact branch join | Exact retained | focused |
| Sema and resolver | `SEM-011` | `arcweft-lang-sema` | unit | different character branch join | Any | focused |
| Sema and resolver | `SEM-012` | `arcweft-lang-sema` | unit | function parameter/return | CharacterDialogue accepted | focused |
| Sema and resolver | `SEM-013` | `arcweft-lang-sema` | unit | closure capture | ordinary capture inventory includes nominal value | focused |
| Sema and resolver | `SEM-014` | `arcweft-lang-sema` | unit | Option/Result/Vec/record | ordinary container type accepted | focused |
| Sema and resolver | `SEM-015` | `arcweft-lang-sema` | unit | indirect content application | target typed CharacterDialogue; accepted | focused |
| Sema and resolver | `SEM-016` | `arcweft-lang-sema` | unit | function coercion | no implicit CharacterDialogue -> Function | focused |
| Sema and resolver | `SEM-017` | `arcweft-lang-sema` | unit | non-character factory intent | AW-CD-001 | focused |
| Sema and resolver | `SEM-018` | `arcweft-lang-sema` | unit | wrong content target | AW-CD-003 | focused |
| Sema and resolver | `SEM-019` | `arcweft-lang-sema` | unit | character patch argument | AW-CD-004 | focused |
| Sema and resolver | `SEM-020` | `arcweft-lang-sema` | unit | duplicate reserved field | AW-CD-005 with two ranges | focused |
| Sema and resolver | `SEM-021` | `arcweft-lang-sema` | unit | inline failure aliases duplicate | AW-CD-006 | focused |
| Sema and resolver | `SEM-022` | `arcweft-lang-sema` | unit | standalone id patch | AW-CD-007 | focused |
| Sema and resolver | `SEM-023` | `arcweft-lang-sema` | unit | outer immediate id patch | accepted application coordinate | focused |
| Sema and resolver | `SEM-024` | `arcweft-lang-sema` | unit | inner id then outer config | inner AW-CD-007 | focused |
| Sema and resolver | `SEM-025` | `arcweft-lang-sema` | unit | unknown custom field | AW-CD-014 | focused |
| Sema and resolver | `SEM-026` | `arcweft-lang-sema` | unit | ambiguous custom binding | ordinary project ambiguity, no first match | focused |
| Sema and resolver | `SEM-027` | `arcweft-lang-sema` | unit | custom value wrong type | AW-CD-015 | focused |
| Sema and resolver | `SEM-028` | `arcweft-lang-sema` | unit | custom None not clearable | AW-CD-016 | focused |
| Sema and resolver | `SEM-029` | `arcweft-lang-sema` | unit | custom field incompatible with new View | AW-CD-022 | focused |
| Sema and resolver | `SEM-030` | `arcweft-lang-sema` | unit | line operation escape | AW-CD-017 | focused |
| Sema and resolver | `SEM-031` | `arcweft-lang-sema` | unit | shared resolver factory fact | one committed typed candidate | focused |
| Sema and resolver | `SEM-032` | `arcweft-lang-sema` | unit | shared resolver reconfigure fact | one committed typed candidate | focused |
| Sema and resolver | `SEM-033` | `arcweft-lang-sema` | unit | shared resolver content fact | bracket/colon authored surface retained | focused |
| Sema and resolver | `SEM-034` | `arcweft-lang-sema` | unit | `.say` on Character | ordinary missing-method diagnostic only | focused |
| Sema and resolver | `SEM-035` | `arcweft-lang-sema` | unit | generic identity function | CharacterDialogue type preserved | focused |
| Config merge | `CFG-001` | `arcweft-dialogue` | unit | voice Unspecified | prior voice preserved | focused |
| Config merge | `CFG-002` | `arcweft-dialogue` | unit | voice Set | new voice replaces prior | focused |
| Config merge | `CFG-003` | `arcweft-dialogue` | unit | voice Clear | effective voice becomes None | focused |
| Config merge | `CFG-004` | `arcweft-dialogue` | unit | look Unspecified | prior look preserved | focused |
| Config merge | `CFG-005` | `arcweft-dialogue` | unit | look Set | new look replaces prior | focused |
| Config merge | `CFG-006` | `arcweft-dialogue` | unit | look Clear | effective look becomes None | focused |
| Config merge | `CFG-007` | `arcweft-dialogue` | unit | stage Unspecified | prior stage preserved | focused |
| Config merge | `CFG-008` | `arcweft-dialogue` | unit | stage Set | new stage replaces prior | focused |
| Config merge | `CFG-009` | `arcweft-dialogue` | unit | stage Clear | effective stage becomes None | focused |
| Config merge | `CFG-010` | `arcweft-dialogue` | unit | portrait Unspecified | prior portrait preserved | focused |
| Config merge | `CFG-011` | `arcweft-dialogue` | unit | portrait Set | new portrait replaces prior | focused |
| Config merge | `CFG-012` | `arcweft-dialogue` | unit | portrait Clear | effective portrait becomes None | focused |
| Config merge | `CFG-013` | `arcweft-dialogue` | unit | focus Unspecified | prior focus preserved | focused |
| Config merge | `CFG-014` | `arcweft-dialogue` | unit | focus Set | new focus replaces prior | focused |
| Config merge | `CFG-015` | `arcweft-dialogue` | unit | focus Clear | effective focus becomes None | focused |
| Config merge | `CFG-016` | `arcweft-dialogue` | unit | cleanup Unspecified | prior cleanup preserved | focused |
| Config merge | `CFG-017` | `arcweft-dialogue` | unit | cleanup Set | new cleanup replaces prior | focused |
| Config merge | `CFG-018` | `arcweft-dialogue` | unit | cleanup Clear | effective cleanup becomes None | focused |
| Config merge | `CFG-019` | `arcweft-dialogue` | unit | source_locale Unspecified | prior source_locale preserved | focused |
| Config merge | `CFG-020` | `arcweft-dialogue` | unit | source_locale Set | new source_locale replaces prior | focused |
| Config merge | `CFG-021` | `arcweft-dialogue` | unit | source_locale Clear | effective source_locale becomes None | focused |
| Config merge | `CFG-022` | `arcweft-dialogue/runtime-plan` | unit | view Unspecified | prior required View preserved | focused |
| Config merge | `CFG-023` | `arcweft-dialogue/runtime-plan` | unit | view Set | new typed View replaces prior | focused |
| Config merge | `CFG-024` | `arcweft-dialogue/runtime-plan` | unit | view Clear | effective View becomes std.view.dialogue | focused |
| Config merge | `CFG-025` | `arcweft-dialogue/runtime-plan` | unit | hooks Unspecified | ordered prior hook list preserved | focused |
| Config merge | `CFG-026` | `arcweft-dialogue/runtime-plan` | unit | hooks Set | complete list replaced in supplied order | focused |
| Config merge | `CFG-027` | `arcweft-dialogue/runtime-plan` | unit | hooks Clear | empty hook list | focused |
| Config merge | `CFG-028` | `arcweft-dialogue/runtime-plan` | unit | style disjoint leaves | field-by-field union | focused |
| Config merge | `CFG-029` | `arcweft-dialogue/runtime-plan` | unit | style same leaf later patch | later value wins | focused |
| Config merge | `CFG-030` | `arcweft-dialogue/runtime-plan` | unit | style leaf Clear | leaf absent | focused |
| Config merge | `CFG-031` | `arcweft-dialogue/runtime-plan` | unit | style clear_all then assignments | empty base then assignments | focused |
| Config merge | `CFG-032` | `arcweft-dialogue/runtime-plan` | unit | style overlapping parent/child same patch | AW-CD-006 | focused |
| Config merge | `CFG-033` | `arcweft-dialogue/runtime-plan` | unit | rich_text disjoint leaves | field-by-field union | focused |
| Config merge | `CFG-034` | `arcweft-dialogue/runtime-plan` | unit | rich_text same leaf later patch | later value wins | focused |
| Config merge | `CFG-035` | `arcweft-dialogue/runtime-plan` | unit | inline failure Unspecified | prior policy preserved | focused |
| Config merge | `CFG-036` | `arcweft-dialogue/runtime-plan` | unit | inline failure Set | new policy replaces | focused |
| Config merge | `CFG-037` | `arcweft-dialogue/runtime-plan` | unit | inline failure Clear | FailLine | focused |
| Config merge | `CFG-038` | `arcweft-dialogue/runtime-plan` | unit | custom distinct keys | both preserved sorted by stable ID | focused |
| Config merge | `CFG-039` | `arcweft-dialogue/runtime-plan` | unit | custom same key later patch | later value replaces | focused |
| Config merge | `CFG-040` | `arcweft-dialogue/runtime-plan` | unit | custom key Clear | key removed | focused |
| Config merge | `CFG-041` | `arcweft-dialogue/runtime-plan` | unit | reserved custom name | AW-CD-004/006 as appropriate | focused |
| Config merge | `CFG-042` | `arcweft-dialogue/runtime-plan` | unit | base immutability | failed/success patch never changes base | focused |
| Config merge | `CFG-043` | `arcweft-dialogue/runtime-plan` | unit | patch evaluation order | left-to-right side effects/evidence order | focused |
| Config merge | `CFG-044` | `arcweft-dialogue/runtime-plan` | unit | patch transaction failure | no candidate or presentation effect | focused |
| Config merge | `CFG-045` | `arcweft-dialogue/runtime-plan` | unit | id None immediate | generated line ID | focused |
| Config merge | `CFG-046` | `arcweft-dialogue/runtime-plan` | unit | text_key None immediate | derived text key | focused |
| Runtime and AWBC | `RUN-001` | `arcweft-core` | unit | nominal record vs anonymous record | not equal and distinct discriminants | focused |
| Runtime and AWBC | `RUN-002` | `arcweft-dialogue` | unit | encode/decode CharacterDialogue | canonical round trip | focused |
| Runtime and AWBC | `RUN-003` | `arcweft-dialogue` | unit | canonical equality/hash | equal bytes and digest | focused |
| Runtime and AWBC | `RUN-004` | `arcweft-dialogue` | unit | source alias/local name differs | runtime equality unchanged | focused |
| Runtime and AWBC | `RUN-005` | `arcweft-dialogue` | unit | negative zero config | normalized to positive zero | focused |
| Runtime and AWBC | `RUN-006` | `arcweft-dialogue` | unit | non-finite config number | rejected | focused |
| Runtime and AWBC | `RUN-007` | `arcweft-core` | unit | nominal depth exactly 64 | accepted | focused |
| Runtime and AWBC | `RUN-008` | `arcweft-core` | unit | nominal depth 65 | AW-CD-R006 | focused |
| Runtime and AWBC | `RUN-009` | `arcweft-core` | unit | wrong nominal type | AW-CD-R001 | focused |
| Runtime and AWBC | `RUN-010` | `arcweft-core` | unit | wrong layout | AW-CD-R002 | focused |
| Runtime and AWBC | `RUN-011` | `arcweft-core` | unit | wrong field count | AW-CD-R003 | focused |
| Runtime and AWBC | `RUN-012` | `arcweft-dialogue` | unit | invalid CharacterId field | AW-CD-R004 | focused |
| Runtime and AWBC | `RUN-013` | `arcweft-dialogue` | unit | invalid View field | AW-CD-R009/R005 | focused |
| Runtime and AWBC | `RUN-014` | `arcweft-dialogue` | unit | noncanonical custom order | AW-CD-R003/R020 | focused |
| Runtime and AWBC | `RUN-015` | `arcweft-dialogue` | unit | duplicate custom ID | hard reject | focused |
| Runtime and AWBC | `RUN-016` | `arcweft-core` | unit | function captures CharacterDialogue | snapshot validation passes | focused |
| Runtime and AWBC | `RUN-017` | `arcweft-core` | unit | collection contains exact 4096 | accepted if container allows | focused |
| Runtime and AWBC | `RUN-018` | `arcweft-core` | unit | collection contains 4097 | AW-CD-R006 | focused |
| Runtime and AWBC | `RUN-019` | `arcweft-core` | unit | function captures 256 values | accepted if ordinary limit allows | focused |
| Runtime and AWBC | `RUN-020` | `arcweft-core` | unit | function captures 257 values | AW-CD-R006 | focused |
| Runtime and AWBC | `RUN-021` | `arcweft-core/awbc` | codec | ABI2 codec8 round trip | byte deterministic | workspace |
| Runtime and AWBC | `RUN-022` | `arcweft-core/awbc` | codec | ABI1/codec7 input | AW-CD-R013; no old reader | workspace |
| Runtime and AWBC | `RUN-023` | `arcweft-core/awbc` | codec | wrong opcode discriminant | AW-CD-R012 | focused |
| Runtime and AWBC | `RUN-024` | `arcweft-core/awbc` | codec | truncated patch table | AW-CD-R012 | focused |
| Runtime and AWBC | `RUN-025` | `arcweft-core/awbc` | verify | duplicate patch coordinate | AW-CD-R014 | focused |
| Runtime and AWBC | `RUN-026` | `arcweft-core/awbc` | verify | Set without register | AW-CD-R015 | focused |
| Runtime and AWBC | `RUN-027` | `arcweft-core/awbc` | verify | Clear with register | AW-CD-R015 | focused |
| Runtime and AWBC | `RUN-028` | `arcweft-core/awbc` | vm | MakeCharacterDialogue dynamic CharacterId | defaults lookup and result | focused |
| Runtime and AWBC | `RUN-029` | `arcweft-core/awbc` | vm | PatchCharacterDialogue | immutable accepted result | focused |
| Runtime and AWBC | `RUN-030` | `arcweft-core/awbc` | vm | field validation fails | dst/base unchanged | focused |
| Runtime and AWBC | `RUN-031` | `arcweft-core/awbc` | vm | patch work exactly 1024 | accepted | focused |
| Runtime and AWBC | `RUN-032` | `arcweft-core/awbc` | vm | patch work 1025 | AW-CD-R016 | focused |
| Runtime and AWBC | `RUN-033` | `arcweft-core/awbc` | verify | Dialogue target register wrong type | verifier rejection | focused |
| Runtime and AWBC | `RUN-034` | `arcweft-core/awbc` | vm | Dialogue suspension stores nominal value | snapshot exact | focused |
| Runtime and AWBC | `RUN-035` | `arcweft-runtime-plan` | unit | checked lowering lacks sema fact | structured fail-closed error | focused |
| Runtime and AWBC | `RUN-036` | `arcweft-runtime-plan` | unit | no callee string fallback | unsupported path rejected | focused |
| Runtime and AWBC | `RUN-037` | `arcweft-runtime-plan` | unit | line plan out result | existing result/handle semantics preserved | focused |
| Runtime and AWBC | `RUN-038` | `arcweft-verify` | unit | DialogueLine escape | verification error | focused |
| Bundle and persistence | `PER-001` | `arcweft-bundle` | codec | AWBC/default/display-schema2 cross-section round trip | deterministic canonical bytes | workspace |
| Bundle and persistence | `PER-002` | `arcweft-bundle` | codec | defaults CharacterId missing manifest | candidate rejected | focused |
| Bundle and persistence | `PER-003` | `arcweft-bundle` | codec | default look wrong owner | candidate rejected | focused |
| Bundle and persistence | `PER-004` | `arcweft-bundle` | codec | default View missing | candidate rejected | focused |
| Bundle and persistence | `PER-005` | `arcweft-bundle` | codec | custom descriptor missing | candidate rejected | focused |
| Bundle and persistence | `PER-006` | `arcweft-bundle` | codec | content/AWBC line mismatch | candidate rejected | focused |
| Bundle and persistence | `PER-007` | `arcweft-runtime-driver` | integration | dynamic CharacterDialogue content | typed display frame | workspace |
| Bundle and persistence | `PER-008` | `arcweft-runtime-driver` | integration | display frame fields | CharacterId/display name; no callee identity | workspace |
| Bundle and persistence | `PER-009` | `arcweft-runtime-driver` | integration | save schema2 round trip in register | exact value restored | workspace |
| Bundle and persistence | `PER-010` | `arcweft-runtime-driver` | integration | save schema2 in closure capture | exact value restored | workspace |
| Bundle and persistence | `PER-011` | `arcweft-runtime-driver` | integration | save schema2 in supported collection | exact values restored | workspace |
| Bundle and persistence | `PER-012` | `arcweft-runtime-driver` | codec | save schema1 input | AW-CD-R013; no old reader | focused |
| Bundle and persistence | `PER-013` | `arcweft-runtime-driver` | codec | tampered nominal value in save | transactional restore reject | focused |
| Bundle and persistence | `PER-014` | `arcweft-runtime-driver` | replay | root replay v1 carries CharacterDialogue in generic RuntimePayload | nominal bytes/digest replay exactly; schema remains 1 | workspace |
| Bundle and persistence | `PER-015` | `arcweft-bundle` | codec | unversioned/old display-catalog transcript | rejected; schema2 is sole reader | focused |
| Bundle and persistence | `PER-016` | `arcweft-runtime-driver` | hot reload | identical contract digests | value rebound/preserved | workspace |
| Bundle and persistence | `PER-017` | `arcweft-runtime-driver` | hot reload | character manifest changed | AW-CD-R007 or old generation retained | workspace |
| Bundle and persistence | `PER-018` | `arcweft-runtime-driver` | hot reload | defaults changed | AW-CD-R008 or old generation retained | workspace |
| Bundle and persistence | `PER-019` | `arcweft-runtime-driver` | hot reload | View removed/contract changed | AW-CD-R009 | workspace |
| Bundle and persistence | `PER-020` | `arcweft-runtime-driver` | hot reload | custom schema changed | AW-CD-R011 | workspace |
| Bundle and persistence | `PER-021` | `arcweft-runtime-driver` | hot reload | active content revision changed | old generation continues; forced resume AW-CD-R010 | workspace |
| Bundle and persistence | `PER-022` | `arcweft-runtime-driver` | hot reload | candidate cross-validation fails | previous accepted generation untouched | workspace |
| Bundle and persistence | `PER-023` | `arcweft-agent-protocol` | codec | Agent observed dialogue round trip | typed character fields and digests | workspace |
| Bundle and persistence | `PER-024` | `arcweft-agent-protocol` | codec | Agent old speaker/callee shape | rejected | focused |
| Bundle and persistence | `PER-025` | `arcweft-player-scene` | integration | View mount receives dynamic frame | same persistent authored View path | tier2 |
| Bundle and persistence | `PER-026` | `arcweft-player-native/web` | parity | native/Web/headless same dialogue frame | exact semantic/render parity | tier2 |
| Tooling | `TOL-001` | `arcweft-tooling` | unit | colon simple canonicalization | bracket output | focused |
| Tooling | `TOL-002` | `arcweft-tooling` | unit | colon configured canonicalization | call head preserved, bracket output | focused |
| Tooling | `TOL-003` | `arcweft-tooling` | unit | Unicode/CRLF/comments | exact safe edits | focused |
| Tooling | `TOL-004` | `arcweft-tooling` | unit | stale sema inventory | no edit + stable diagnostic | focused |
| Tooling | `TOL-005` | `arcweft-tooling` | unit | non-dialogue colon | no dialogue edit | focused |
| Tooling | `TOL-006` | `arcweft-tooling` | unit | canonical output scan by behavior | result contains direct bracket and no `.say` | focused |
| Tooling | `TOL-007` | `arcweft-lsp` | integration | factory completion | standard/custom typed fields | focused |
| Tooling | `TOL-008` | `arcweft-lsp` | integration | reconfigure completion | same schema without id/text_key | focused |
| Tooling | `TOL-009` | `arcweft-lsp` | integration | immediate application completion | id/text_key available | focused |
| Tooling | `TOL-010` | `arcweft-lsp` | integration | exact look completion | correct Character inventory | focused |
| Tooling | `TOL-011` | `arcweft-lsp` | integration | Any look completion | no unqualified first-match variants | focused |
| Tooling | `TOL-012` | `arcweft-lsp` | integration | signature help in `(` | configuration signature | focused |
| Tooling | `TOL-013` | `arcweft-lsp` | integration | signature help in `[` | content signature | focused |
| Tooling | `TOL-014` | `arcweft-lsp` | integration | signature help at colon/with | content/line-plan result | focused |
| Tooling | `TOL-015` | `arcweft-lsp` | integration | hover exact/any | CharacterDialogue type and config facts | focused |
| Tooling | `TOL-016` | `arcweft-lsp` | integration | definition character/look/view/custom/line | typed source targets | focused |
| Tooling | `TOL-017` | `arcweft-lsp` | integration | character rename | generated/explicit line IDs unchanged | focused |
| Tooling | `TOL-018` | `arcweft-lsp` | integration | line ID rename | Character refs unchanged | focused |
| Tooling | `TOL-019` | `arcweft-lsp` | integration | semantic tokens | new character-dialogue roles | focused |
| Tooling | `TOL-020` | `arcweft-lsp` | integration | code actions | no `.say` insertion action | focused |
| Tooling | `TOL-021` | `arcweft-cli` | integration | `arcw fmt` | syntax-only, never introduces `.say` | workspace |
| Tooling | `TOL-022` | `arcweft-cli` | integration | `arcw canonicalize` | project-aware bracket expansion | workspace |
| Tooling | `TOL-023` | `arcweft-cli` | integration | canonicalization unavailable | no write | focused |
| Limits | `LIM-001` | `owning crate` | unit/codec | max_patch_fields exact 64 | accepted | focused |
| Limits | `LIM-002` | `owning crate` | unit/codec | max_patch_fields one over 65 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-003` | `owning crate` | unit/codec | max_custom_fields exact 32 | accepted | focused |
| Limits | `LIM-004` | `owning crate` | unit/codec | max_custom_fields one over 33 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-005` | `owning crate` | unit/codec | max_custom_field_id_bytes exact 128 | accepted | focused |
| Limits | `LIM-006` | `owning crate` | unit/codec | max_custom_field_id_bytes one over 129 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-007` | `owning crate` | unit/codec | max_hooks exact 64 | accepted | focused |
| Limits | `LIM-008` | `owning crate` | unit/codec | max_hooks one over 65 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-009` | `owning crate` | unit/codec | max_config_string_bytes exact 16384 | accepted | focused |
| Limits | `LIM-010` | `owning crate` | unit/codec | max_config_string_bytes one over 16385 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-011` | `owning crate` | unit/codec | max_locale_bytes exact 64 | accepted | focused |
| Limits | `LIM-012` | `owning crate` | unit/codec | max_locale_bytes one over 65 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-013` | `owning crate` | unit/codec | max_structured_depth exact 8 | accepted | focused |
| Limits | `LIM-014` | `owning crate` | unit/codec | max_structured_depth one over 9 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-015` | `owning crate` | unit/codec | max_structured_leaves exact 256 | accepted | focused |
| Limits | `LIM-016` | `owning crate` | unit/codec | max_structured_leaves one over 257 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-017` | `owning crate` | unit/codec | max_fx_applications exact 128 | accepted | focused |
| Limits | `LIM-018` | `owning crate` | unit/codec | max_fx_applications one over 129 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-019` | `owning crate` | unit/codec | max_field_value_bytes exact 65536 | accepted | focused |
| Limits | `LIM-020` | `owning crate` | unit/codec | max_field_value_bytes one over 65537 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-021` | `owning crate` | unit/codec | max_config_encoded_bytes exact 524288 | accepted | focused |
| Limits | `LIM-022` | `owning crate` | unit/codec | max_config_encoded_bytes one over 524289 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-023` | `owning crate` | unit/codec | max_values_per_sequence exact 4096 | accepted | focused |
| Limits | `LIM-024` | `owning crate` | unit/codec | max_values_per_sequence one over 4097 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-025` | `owning crate` | unit/codec | max_captured_values_per_function exact 256 | accepted | focused |
| Limits | `LIM-026` | `owning crate` | unit/codec | max_captured_values_per_function one over 257 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-027` | `owning crate` | unit/codec | max_defaults_entries exact 4096 | accepted | focused |
| Limits | `LIM-028` | `owning crate` | unit/codec | max_defaults_entries one over 4097 | stable limit diagnostic/rejection | focused |
| Limits | `LIM-029` | `owning crate` | unit/codec | max_line_id_bytes exact 256 | accepted | focused |
| Limits | `LIM-030` | `owning crate` | unit/codec | max_line_id_bytes one over 257 | stable limit diagnostic/rejection | focused |
| Deletion and architecture | `DEL-001` | `arcweft-dialogue` | compile-fail | SpeakerRef | name/API unavailable | workspace |
| Deletion and architecture | `DEL-002` | `arcweft-dialogue` | compile-fail | SpeakerPreset | name/API unavailable | workspace |
| Deletion and architecture | `DEL-003` | `arcweft-dialogue` | compile-fail | SayOptions | name/API unavailable | workspace |
| Deletion and architecture | `DEL-004` | `arcweft-dialogue` | compile-fail | Character.say | method unavailable | workspace |
| Deletion and architecture | `DEL-005` | `arcweft-dialogue` | compile-fail | SpeakerPreset.say/call | method/type unavailable | workspace |
| Deletion and architecture | `DEL-006` | `arcweft-lang-sema` | compile-fail | TypeKind::Speaker | variant unavailable | workspace |
| Deletion and architecture | `DEL-007` | `arcweft-lang-sema` | compile-fail | TypeKind::SpeakerPreset | variant unavailable | workspace |
| Deletion and architecture | `DEL-008` | `arcweft-lang-sema` | compile-fail | DialogueCalleeIdentity old variants | variants unavailable | workspace |
| Deletion and architecture | `DEL-009` | `arcweft-lang-sema` | compile-fail | DialogueCallableId::SpeakerLine | variant unavailable | workspace |
| Deletion and architecture | `DEL-010` | `arcweft-runtime-plan` | behavior | preset let/string reconstruction fixture | typed runtime value used; no fallback | workspace |
| Deletion and architecture | `DEL-011` | `arcweft-tooling` | behavior | colon canonicalization | never emits `.say` | workspace |
| Deletion and architecture | `DEL-012` | `arcweft-lang-syntax/sema` | behavior | authored `.say` | ordinary missing-method rejection | workspace |
| Deletion and architecture | `DEL-013` | `workspace` | dependency | CSS/Takumi route | no dependency/path added; cargo metadata architecture test | audit |
| Deletion and architecture | `DEL-014` | `workspace` | architecture | no source gate | tests use APIs/codecs/compile-fail only | audit |
| Validation | `VAL-001` | `workspace` | command | cargo fmt --all -- --check | pass | workspace |
| Validation | `VAL-002` | `workspace` | command | cargo check --workspace --all-targets --all-features | pass | workspace |
| Validation | `VAL-003` | `workspace` | command | cargo clippy --workspace --all-targets --all-features -- -D warnings | pass | workspace |
| Validation | `VAL-004` | `workspace` | command | just test-workspace | pass | workspace |
| Validation | `VAL-005` | `workspace` | command | just test-doc | pass where policy requires | workspace |
| Validation | `VAL-006` | `workspace` | command | just test-tier2 | Agent/MCP/render/save/replay/hot reload pass | tier2 |
| Validation | `VAL-007` | `workspace` | command | canonical structure audit | zero error-level violations or documented resolved blockers | audit |
| Validation | `VAL-008` | `workspace` | command | git diff --check | pass | workspace |

**Normative rows:** 260

Every actual codec owner must additionally exercise wrong discriminant, duplicate, truncated, oversized, stale, and noncanonical input even when that exact corruption is represented by a sibling row.
