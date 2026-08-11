# Requirements traceability

## Required output decisions

| Requirement | Closure | Location |
|---|---|---|
| 1. Preserve `DialogueProfileSpec`, `DialoguePresentationProfile`, decoder, source-map owners | closed; launch/dialogue owners retained | `FINAL_CONTRACT.md` §§2–4; `OWNER_AND_DEPENDENCY_GRAPH.md` |
| 2. Stable consumer-facing source-map access without parallel map | `SourceBackedManifest::manifest_token_span` with typed path/slot; raw map crate-private | `FINAL_CONTRACT.md` §9; `SOURCE_MAP_AND_DIAGNOSTICS.md` |
| 3. Cycle-free Cargo graph through admission owner | compiler owns checked admission; reusable revision in lower dialogue crate; no project-loader/runtime-driver reversal | `OWNER_AND_DEPENDENCY_GRAPH.md` |
| 4. Exact construction order | decode → resolve → topology freeze → compiler product → admission → revision → compiled project → runtime plan → atomic generation | `ACCEPTED_CANDIDATE_FLOW.md` |
| 5. Exact checked type and owner | `arcweft-compiler::project::dialogue_profile::CheckedDialogueProfile` with six retained fields | `FINAL_CONTRACT.md` §5; `AS_BUILT_API.md` |
| 6. Exact revision tuple/equality | six typed fields; derived structural equality; admission Arc and product coherence | `FINAL_CONTRACT.md` §8; `ACCEPTED_CANDIDATE_FLOW.md` |
| 7. Diagnostic owner/code/primary/related data | launch shape/family; compiler admission codes, ranges, single non-dialogue secondary | `SOURCE_MAP_AND_DIAGNOSTICS.md` |
| 8. Omitted/partial/complete TOML | exact schema-1 examples, kebab-case table, strict tagged policies | `FINAL_MANIFEST_SCHEMA.md` |
| 9. Direct migration order | nine compileable increments, deletion-driven cut, atomic closure | `MIGRATION_AND_DELETION.md` |

## Required tests

| Request test | Matrix ID(s) |
|---|---|
| neutral manifest dependency graph | T01, T30 |
| one decoder invocation reused by all consumers | T02, T19, T25 |
| exact dialogue field ranges | T03 |
| omitted/view-only/style-only/policy-only/complete | T04–T08 |
| fixed `inline-failure` and rejected spelling | T09 |
| missing/wrong-family/non-dialogue View | T10–T12 |
| missing/wrong-family Style | T13–T14 |
| manifest/product build revision mismatch | T15, T29 |
| rejected publication retains previous tuple | T18, T27 |
| CLI and LSP same checked candidate/no reparse | T19 |
| runtime/native/Web/headless/Agent/MCP parity | T20, T28 |
| ordinary parser rejection and typed-node absence | T21 |
| revision/save/codec identity | T22, T26 |

## Constraints

| Constraint | Enforcement |
|---|---|
| do not move decoder | owner table and forbidden-edge graph |
| do not make manifest model presentation facade | dependency graph T01 |
| do not make project-loader depend runtime-driver | forbidden edge and T01/T30 |
| do not duplicate catalogs/maps/registries/revisions | final contract §§9, 13; flow; T02/T18/T30 |
| do not restore removed syntax/features | deletion inventory and T21 |
| do not redesign accepted Character/View/Style/prepared-text substrate | explicit scope/non-goals |
| no raw strings/prefix checks/local conversion traits/helpers/wrappers | nominal decoder boundary and migration prohibitions |
| no source gate | structured behavior/codec/Cargo metadata verification |
| atomic rollback | publication contract and T18/T27 |

## Archive requirements

All requested mandatory files are present. Additional files make the resolved
request disposition, exact current API, Japanese summary, and verification
scope explicit. `OPEN_QUESTIONS.md` is exactly `none
`.

The historical machine field for Jujutsu is intentionally not reproduced as a
change ID because current repository policy is Git-only. The full Git SHA is
recorded everywhere that implementation evidence depends on a revision.
