# Requirements traceability

| Request requirement | Final decision | Primary evidence in archive |
| --- | --- | --- |
| select exactly one outcome | `READY_FOR_IMPLEMENTATION` | `README.md`; `FINAL_STATUS.md`; `FINAL_CONTRACT.md` §1 |
| exhaustive native surface inventory | parenthesized expression, presentation, dialogue, source/project, adapter, methods, values, partials, overloads, flows, constructors, and recovery all classified | `SURFACE_INVENTORY.md` §§2–7 |
| prove character nominal applicability | `show.look`, dialogue `look`, and accepted typed signatures | `FINAL_CONTRACT.md` §§1, 11; `SURFACE_INVENTORY.md` §§3–4 |
| one canonical semantic query | one public `arcweft-lang-sema::signature::query_signature` | `FINAL_CONTRACT.md` §5 |
| consume accepted world and symbols | request takes `RegisteredSemanticWorld`; LSP passes `AcceptedProfileEnvironment::world()`; registered callable facts retained atomically | `FINAL_CONTRACT.md` §§5–8 |
| no parallel TypeCheckEnv authority | base remains private; no `profile.typecheck_env()` in signature path | `FINAL_CONTRACT.md` §§6–8; `IMPLEMENTATION_HANDOFF.md` cuts 2–4 |
| base/adapter/project/presentation/method/overload/function-value normalization | one `SignatureCandidateId` and `ResolvedCallTarget` | `FINAL_CONTRACT.md` §§5.3, 7 |
| select one source/syntax identity path | exact `SourceDocumentIdentity` plus parser-retained ranges; no snapshot/node dependency | `FINAL_CONTRACT.md` §§3–4, 8; `IDENTITY_CACHE_AND_LIMITS.md` §1 |
| decide proof 01.1.1 dependency | not a prerequisite | `README.md`; `IMPLEMENTATION_HANDOFF.md` §1 |
| exact LSP/UTF-8 conversion | checked `LineIndex` method; no clamping | `FINAL_CONTRACT.md` §15; `TEST_MATRIX.md` I01–I05 |
| exact call selection and nested precedence | range containment and deterministic tie rules | `FINAL_CONTRACT.md` §9; `TEST_MATRIX.md` R01–R03 |
| active argument boundaries | opening/closing/comma/whitespace/trailing rules frozen | `FINAL_CONTRACT.md` §10.1; `TEST_MATRIX.md` R04–R12 |
| named/reordered/positional/spread/partial behavior | per-candidate binding algorithm frozen | `FINAL_CONTRACT.md` §10.2; `TEST_MATRIX.md` A01–A14 |
| missing delimiter and poisoned recovery | parser-retained recovery plus partial/error split | `FINAL_CONTRACT.md` §§3.3, 13; `TEST_MATRIX.md` R13–R16 |
| exact typed parameters | `Known(TypeKind)`, `Unconstrained`, or typed `Unavailable`; no fake `Named("_")` | `FINAL_CONTRACT.md` §§5.2, 11 |
| same spelling across owners/families/parts | structural `CharacterNominalType` equality only | `FINAL_CONTRACT.md` §11; `TEST_MATRIX.md` C05–C09 |
| alias policy | authored alias display only; canonical nominal type label; no inverse parse | `FINAL_CONTRACT.md` §11.4; `TEST_MATRIX.md` C01–C04 |
| labels/documentation/parameter ranges | deterministic label grammar, doc priority, UTF-16 offsets | `FINAL_CONTRACT.md` §12; `TEST_MATRIX.md` O09–O12 |
| active signature and overload ordering | viability/specificity rules and typed ordering | `FINAL_CONTRACT.md` §§10.3, 12.4; `TEST_MATRIX.md` O01–O08 |
| duplicate coalescing | exact typed ID/signature/provenance equality only | `FINAL_CONTRACT.md` §12.4; `TEST_MATRIX.md` O07–O08 |
| native/Rust adapter precedence | adapter normalized; project wins; corrupt same-rank authority fails | `FINAL_CONTRACT.md` §§7.2–7.3; `TEST_MATRIX.md` P01–P10 |
| delete word fallback and dual resolvers | explicit cut-5 deletion | `IMPLEMENTATION_HANDOFF.md` cut 5 |
| accepted generation/cache key | typed full key and final stamp | `IDENTITY_CACHE_AND_LIMITS.md` §§2–3 |
| failed rebuild behavior | no publication or candidate key; old world/cache atomic; changed source stale | `IDENTITY_CACHE_AND_LIMITS.md` §5; `TEST_MATRIX.md` I16–I17 |
| invalidation for profile/manifest/doc close/workspace/shutdown | event matrix frozen | `IDENTITY_CACHE_AND_LIMITS.md` §5 |
| typed errors and outcome mapping | sema and LSP error enums plus table | `FINAL_CONTRACT.md` §§13–14 |
| unknown owner/part | partial help with unavailable type and structured diagnostic | `FINAL_CONTRACT.md` §§11, 13; `TEST_MATRIX.md` C10–C12 |
| ambiguity | overload is partial; same-rank authority is request failure | `FINAL_CONTRACT.md` §§7.3, 10.3, 13 |
| cancellation/timeout/resource | checked control and LSP mappings | `FINAL_CONTRACT.md` §§5.1, 13–14; `TEST_MATRIX.md` L17–L19 |
| exact inclusive limits | 4096/64/128/64/512/8 MiB/32/262144 | `IDENTITY_CACHE_AND_LIMITS.md` §§6–8 |
| deterministic truncation versus fail closed | diagnostics only truncate; all result-changing collections fail closed | `IDENTITY_CACHE_AND_LIMITS.md` §8 |
| arithmetic overflow | typed error, no cache | `IDENTITY_CACHE_AND_LIMITS.md` §§7–9; `TEST_MATRIX.md` L15–L16 |
| implementation order | six compiling cuts, proof dependency decision, deletion and validation | `IMPLEMENTATION_HANDOFF.md` §1 |
| mandatory direct tests | every requested positive, negative, recovery, stale, precedence, exact, one-over, deterministic, and no-bypass case mapped | `TEST_MATRIX.md` |
| no automated source gates | direct behavior/type/dependency evidence only | `TEST_MATRIX.md` opening and §9; `IMPLEMENTATION_HANDOFF.md` §6 |
| fixed substrate/non-redesign boundary | canonical character/world/symbol/source owners retained; completion/hover/proof untouched | `FINAL_CONTRACT.md` §§2, 6, 16 |
| no production changes in design delivery | archive contains Markdown/manifest only | `README.md`; `REPOSITORY_EVIDENCE.md`; `FINAL_STATUS.md` |
| required ZIP membership and open questions | all required implementation-outcome files; `OPEN_QUESTIONS.md` is exactly `none` | `MANIFEST.txt`; `OPEN_QUESTIONS.md` |
| integrity artifacts | sorted manifest with zero self-entry; external ZIP digest/status/summary | `MANIFEST.txt` and sidecar files |

## Acceptance closure

Every result-changing choice is fixed. The implementation may choose internal
function decomposition and local variable names only when those choices do not
change the public types, resolver ownership, precedence, range semantics,
active indices, error variants, limits, ordering, invalidation, deletion scope,
or tests above.
