# Requirements traceability

## 1. Required decisions

| Request requirement | Selected decision | Normative location | Direct evidence |
|---|---|---|---|
| 1. Complete public model | exact scalar/path/provider IDs; all candidate IDs; `SignatureOrigin`; docs/source/Rust provenance; schema; resolved products; facts; public results; visibility | `FINAL_CONTRACT.md` §§2–11, 17, 24–27, 35 | `TEST_MATRIX.md` §§1–2, 15, 18 |
| 2. Catalog record and key | `CallableRecord`, `CallableLookupKey`, `ReceiverMethodKey`, groups/params/default/rest/names/effects/docs/spans, rank/provider/order, non-empty sets, typed errors | `FINAL_CONTRACT.md` §§3, 10–11, 14 | tests §§1, 3–4, 14, 17 |
| 3. Project publication | HIR-owned source row; all modules including empty; callable/non-callable binding map; same atomic transaction; no source impl synthesis | `FINAL_CONTRACT.md` §12; `PRODUCTION_RECONCILIATION.md` §5 | tests §3 |
| 4. Adapter identity | manifest `id` only; six typed standard owners; core owner; typed normalization; Rust/docs/effects preserved; no display parsing | `FINAL_CONTRACT.md` §§4, 13; reconciliation §6 | tests §4 |
| 5. Every resolver family | exhaustive free/selected candidate IDs, signature source, applicability, precedence, result/effect/check behavior | `SURFACE_INVENTORY.md` §§1–19; `FINAL_CONTRACT.md` §§7–9, 19–23 | tests §§5–13 |
| 6. Presentation/dialogue schemas | exact inherent APIs; typed owner acquisition; structural look; open-name policies; deterministic owner failures | `FINAL_CONTRACT.md` §§7.5–7.6, 18, 21–22 | tests §§9–10 |
| 7. Resolver input/product | exact borrowed lexical/expected/receiver/world/module/source/group/cancel/work request; validated non-empty products | `FINAL_CONTRACT.md` §§16–17, 19–20 | tests §§1, 11–13, 16 |
| 8. Checker target-fact mode | one checker invocation; focused/all/disabled; candidate/arg/function/group/result/effect/poison facts; no world mutation | `FINAL_CONTRACT.md` §§24, 30 | tests §15 and final migration evidence |
| 9. Argument checking ownership | one transactional common mapper; explicit retained family validators; one selected commit; no validator name lookup | `FINAL_CONTRACT.md` §23; reconciliation §§7–8 | tests §§5–14 |
| 10. Ambiguity/collisions | standard tie wins; exact coalescing; same-rank build rejection; project non-callable shadow; trait/data-last/capacity behavior; corrupt fail-closed | `FINAL_CONTRACT.md` §§14, 19–20, 23, 29, 32 | tests §§4, 12–13, 16–17 |
| 11. Limits | exact inclusive constants; per-loop charges; build/query domains; checked arithmetic; no truncation/partial/cache | `FINAL_CONTRACT.md` §28 | tests §16 |
| 12. Public result invariants | validating constructors/accessors for signatures/help/diagnostics/work/limits; active/source checks | `FINAL_CONTRACT.md` §§25–28 | tests §§15–16, 18 |

## 2. Required implementation order

| Required order | Handoff cut |
|---|---|
| 1. IDs/records/schema/errors/tests | `IMPLEMENTATION_HANDOFF.md` §2 |
| 2. atomic project/standard/adapter publication | handoff §3 |
| 3. one free resolver family at a time, old branch unreachable/deleted | handoff §4 |
| 4. every selected/method family | handoff §5 |
| 5. presentation/dialogue structural expectations | handoff §§4.5, 6 |
| 6. target facts/public results | handoff §7 |
| 7. connect .3.1 then .3.2 carriers | handoff §8 |
| 8. focused/workspace/audit validation | handoff §9 |

## 3. Mandatory direct tests

| Requested test group | Test matrix coverage |
|---|---|
| every current free-call and selected family | §§5–13, plus complete enum/table parameterization in §2 |
| project callable/non-callable, standard, adapter, overloads, duplicate ID, collision | §§3–4 |
| project/adapter/Rust docs/provenance/default/rest/named/spans | §§3–4 |
| show/dialogue canonical/compact/qualified/alias and missing/non-character/unknown part | §§9–10 |
| same local nominal spelling across characters/families/parts | §§9–10 |
| enum/Result, function values, locals, partial/curried | §§6, 11 |
| environment/builtin/trait/data-last/capacity precedence | §§12–13 |
| reversed insertion order | §§4, 17 |
| checker/signature same candidate | §§6, 9–10, 15, 19 |
| dependency/visibility through APIs/Cargo metadata | §18 |

## 4. Fixed constraints and non-goals

| Constraint | Enforcement |
|---|---|
| do not redesign `CharacterNominalType` | existing structural variants consumed unchanged; `FINAL_CONTRACT.md` §18 |
| do not redesign source identity | exact existing `SourceDocumentIdentity`/`SourceSpan` validation; §§5, 16, 25 |
| do not redesign accepted publication | catalog joins existing atomic registered-world transaction; §15 |
| do not select .3.1/.3.2 carriers | scope §§1, 16, 30; handoff §8 |
| no old branches as fallback | deletion rule §33; handoff §§4–5, 10 |
| no signature-only resolver | shared protocol §§24, 30, 33 |
| no source impl publication without proof | §12 and inventory §20 |
| no label/alias/Rust display/comment/source parsing | §§2–7, 13, 18, 25, 34; reconciliation §§5–6 |
| no extension traits/helpers around owned enums | exact inherent APIs; handoff §§2, 4–5 |
| no compatibility/deprecation/source gates | §33; reconciliation §12; handoff §§10–11 |
| no CSS/Takumi/removed syntax | inventory §20; reconciliation §12 |

## 5. Output contract

| Artifact requirement | Package evidence |
|---|---|
| exact ZIP name | outside status and SHA sidecars; final delivery link |
| required eleven members | ZIP verification and `MANIFEST.txt` |
| `OPEN_QUESTIONS.md` exactly `none` | exact byte check in package verification |
| sorted verified manifest | generated after all other members; self-entry zero rule |
| summary/status/SHA sidecars | generated beside ZIP and cross-checked |
| READY only with zero decisions | `FINAL_STATUS.md`; `OPEN_QUESTIONS.md`; complete sections above |

## 6. Upstream AW-AH-009.3 reconciliation

The upstream package selected one native semantic signature query and structural
character nominal typing. This contract keeps those decisions and fills only
the production seams identified by AW-AH-009.3.3:

- opaque candidate IDs become exact owned types;
- unspecified origins/resolved products become validating public types;
- presentation/dialogue methods receive exact signatures and schema products;
- registered callable state becomes an immutable atomic catalog;
- adapter identity/provenance receives one typed normalization route;
- every current method/free family is classified;
- checker target facts become the single signature-help source;
- collision, ambiguity, limit, and public invariant behavior is frozen.

No result-changing decision is delegated to the implementer.
