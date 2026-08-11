# RESOLUTION AND POISON TABLE

## 1. Precedence table

The resolver evaluates one path/head in the exact order below. “Stop” means no
lower-precedence source may be consulted.

| Order | Typed condition | Resolution product | Semantic type | Diagnostic | Poison | Fallback |
|---:|---|---|---|---|---|---|
| 1 | `TypeRef::Recovery(id)` | `Poisoned(recovery poison)` | `TypeKind::Error` | none from nominal resolver | reused syntax poison | stop |
| 2 | exact unqualified `Self`, `SelfTypeScope::Known` | `SelfType` | supplied checked Self type | none | no | stop |
| 2 | exact unqualified `Self`, scope poisoned | `Poisoned` | `Error(existing)` | none | existing | stop |
| 2 | exact unqualified `Self`, scope absent | `Failed(SelfUnavailable)` | `Error(new)` | `sema.nominal.self_unavailable` | authoritative | stop |
| 3 | unqualified one-segment name in nearest generic frame | `Generic(id)` | `GenericParam(id)` | none | no | stop |
| 4 | reserved built-in path and correct arity/kind | `Builtin` | existing built-in `TypeKind` | none | child poisons only | stop |
| 4 | reserved built-in wrong arity | `Failed(WrongArity)` | `Error(new)` | `sema.nominal.wrong_arity` | authoritative | stop |
| 4 | `Array<T, N>` with non-const `N` | `Failed(WrongKind)` at argument | outer recovery shape with error | `sema.nominal.wrong_kind` | authoritative for argument | stop |
| 5 | project table selects struct/enum with correct arity | `Project` | `ProjectNominal(id,args)` | none | child poisons only | stop |
| 5 | project table selects alias with correct arity and acyclic target | `Alias` plus trace | normalized target | target/child diagnostics only | as target | stop |
| 5 | project table selects source-backed external type | `External` | exact/accepted/character type | none | child poisons only | stop |
| 5 | project table ambiguous | `Failed(Ambiguous)` | `Error(new)` | `sema.nominal.ambiguous_type` | authoritative | stop |
| 5 | project target exists but is inaccessible | `Failed(Inaccessible)` | `Error(new)` | `sema.nominal.inaccessible_type` | authoritative | stop |
| 5 | selected target is callable/module/non-type external | `Failed(WrongKind)` | `Error(new)` | `sema.nominal.wrong_kind` | authoritative | stop |
| 5 | project table says unknown | continue | — | — | — | exact environment |
| 6 | exact accepted record, correct arity | `Accepted` | exact or `AcceptedNominal` | none | child poisons only | stop |
| 6 | exact accepted record, wrong arity | `Failed(WrongArity)` | `Error(new)` | `sema.nominal.wrong_arity` | authoritative | stop |
| 7 | exactly one explicit open rule matches | `Open` | `OpenNominal(rule,path,args)` | none | child poisons only | stop |
| 7 | open rule path matches, arity does not | `Failed(WrongArity)` | `Error(new)` | `sema.nominal.wrong_arity` | authoritative | stop |
| 8 | accepted world, no evidence | `Failed(Unknown)` | `Error(new)` | `sema.nominal.unknown_type` | authoritative | stop |
| 9 | detached world, no project proof | `DetachedUnavailable` | `TypeKind::Error` with a detached-unavailable poison | no project diagnostic | non-authoritative | stop |

Qualified paths skip orders 2–4. A generic or `Self` name is never matched by
suffix. Project ambiguity/inaccessibility/wrong-kind never falls through.

## 2. Recursive node table

| `TypeRef` form | Resolver action | Recovered shape after child failure | Additional owner |
|---|---|---|---|
| `Never` | built-in atomic | `Never` | none |
| `ConstInt` | const argument fact | const value | only valid where constructor permits |
| `Path` | precedence table | selected type or `Error` | project/env/open |
| `Tuple` | resolve every item in order | tuple containing child errors | tuple compatibility |
| `Function` | resolve all params then return | function containing child errors | existing effect-row authority checks effects |
| `Choice` | resolve all alternatives, flatten/normalize, then duplicate check | choice excluding error alternatives from duplicate comparison | existing anonymous-choice diagnostic |
| `Generic` | resolve arguments first, then head/arity/expansion | constructor/project type or error | alias/builtin/env |
| `TraitBound` | record typed trait head; resolve every type argument and associated binding value | typed bound with child errors | existing trait authority selects/validates the trait head |
| `Projection` | resolve subject; preserve member token | projection with subject error | existing trait checker validates member |
| `Reference` | resolve referent; retain borrow/lifetime evidence | reference containing child error | existing borrow checker |
| `Slice` | resolve item | slice containing child error | existing type checker |
| `Recovery` | map to existing poison | `Error(existing)` | parser/syntax diagnostic owner |

## 3. Project target table

| Project target | Type-position result | Related source |
|---|---|---|
| `Nominal(Struct)` | project nominal type | declaration name; imported/re-export sites |
| `Nominal(Enum)` | project nominal type | declaration name; imported/re-export sites |
| `Nominal(TypeAlias)` | alias expansion | alias declaration, target, use/import sites |
| `External` with accepted environment type owner | external accepted type | external declaration and owner source |
| `External` with character owner | existing `CharacterNominalType` | external declaration and character definition |
| `External` with non-type owner | wrong kind | external declaration/owner |
| `Callable` | wrong kind | callable declaration |
| `Module` | wrong kind | module/import source |
| multiple distinct targets | ambiguous | every candidate declaration and binding site |
| hidden target only | inaccessible | hidden declaration and blocking binding |
| no target | environment/open lookup, then unknown/unavailable | authored head |

## 4. Alias normalization table

| Condition | Action | Diagnostic/poison |
|---|---|---|
| zero-parameter alias used as path | expand | none unless target fails |
| parameterized alias used as path | actual arity 0 | wrong arity, poison alias node |
| exact arguments | resolve all args, bind by typed parameter ID, substitute | target diagnostics propagate |
| too few/many arguments | still resolve authored arguments; do not expand target | one wrong-arity poison at head plus independent argument errors |
| imported/re-exported alias | select original alias ID | same expansion facts as qualified use |
| alias chain | append one fact per declaration | target poison is shared/deduplicated |
| alias re-enters same ID | canonical ID cycle | cyclic-alias diagnostic and poison |
| same name in another module | distinct ID | no cycle unless actual ID repeats |
| argument spelling equals parameter name elsewhere | typed ID prevents capture | no special case |
| normalized result is anonymous choice | run existing duplicate checker after full expansion | existing choice diagnostic only |
| target unknown | declaration-owned unknown diagnostic | alias and dependent uses carry target poison |
| target detached/unavailable | mark detached partial | no fabricated project diagnostic |

## 5. Poison suppression matrix

“Suppress” applies only when the stated downstream diagnostic requires the
poisoned fact. Unrelated checks continue.

| Poison source | Suppress | Do not suppress |
|---|---|---|
| unknown/ambiguous/inaccessible/wrong-kind return annotation | Try/Await target missing, non-result boundary, propagation error mismatch, return mismatch derived from that boundary | operand type/effect errors, unrelated statements, sibling signatures |
| wrong alias arity at return annotation | same propagation/boundary cascades | errors in provided alias arguments |
| cyclic alias in return annotation | same propagation/boundary cascades | independent body errors |
| unknown operand type inside Try/Await | propagation diagnostics needing operand error/success shape | boundary diagnostics independent of operand and unrelated expressions |
| syntax recovery type node | diagnostics requiring recovered node type | parser diagnostic and sibling type checks |
| projection subject poison | associated-member-not-found and projection mismatch derived from subject | unrelated bounds and sibling projections |
| field type poison | field/type compatibility derived from that field | other fields and declarations |
| one choice alternative poison | duplicate comparison involving that alternative | duplicate checks among complete alternatives |
| limit/work poison | all results requiring unvisited nodes | already completed independent nodes |
| detached unavailable | all claims that a project name is known/unknown | built-in/generic/Self/exact detached environment checks |

## 6. TM/RD outcomes

| Case | Required exact result |
|---|---|
| TM-072 / RD-084 | Generic alias selected by declaration ID, arguments substituted, normalized `Result` boundary used; no alias-name branch |
| TM-074 | Alias-normalized duplicate anonymous choices emit only the pre-existing choice duplicate diagnostic |
| TM-080 | Unknown return path emits only `sema.nominal.unknown_type`; checked boundary is `Unresolved`; Try success recovery survives; no `sema.try.*` or propagating-Await cascade |
| TM-083 | Generic result alias expands through alias ID and substituted arguments; renaming the alias does not change semantics |
| Prefix Try counterpart | Same boundary poison and recovery discipline as postfix Try |
| Propagating Await counterpart | Same boundary poison and recovery discipline; typed Await source remains unchanged |
