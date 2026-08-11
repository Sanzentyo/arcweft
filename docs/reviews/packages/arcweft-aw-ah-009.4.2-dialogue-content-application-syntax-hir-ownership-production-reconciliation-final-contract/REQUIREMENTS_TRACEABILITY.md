# Requirements traceability

## 1. Required decisions

| Request obligation | Final decision | Normative owner | Direct tests |
|---|---|---|---|
| exact inline/indented/missing carrier; base/body/ending/dedent; tabs/mixed whitespace; CRLF/Unicode/comments/trivia; constructors/derives/Serde | D-008 through D-010 and complete public/private shapes | `COLON_INDENTATION_MODEL.md` | 2–9, 13, 33–35, 88, 90 |
| one generic lossless postfix CST; target/payload/delimiters/trivia/plan/identity | D-001 through D-004 | `POSTFIX_BRACKET_CST_AST.md` §§1–2,5–6 | 17–18, 23–26, 29–32, 36–37 |
| exact AST variants for index/dialogue/ambiguous/invalid/recovered/colon | D-005 through D-007 | `POSTFIX_BRACKET_CST_AST.md` §§3–4 | 1, 11–13, 17–24, 27–30 |
| typed HIR application and unresolved postfix variants; ExprId ownership; callable/flow scopes; source-backed/synthetic; poison | D-016 through D-022 | `TYPED_HIR_OWNERSHIP.md` | 52–80 |
| ordinary call composition; immediate coordinates; three IdRef forms; runtime distinction; duplicates; malformed values | D-012 through D-015 | `CALL_AND_IDREF_INTEGRATION.md` | 10, 23–24, 38–51, 79–80 |
| exact recovery/ambiguity/plan cases and diagnostics | D-003, D-005, D-011, D-022 | `RECOVERY_AND_LIMITS.md` §§1–3 | 10–16, 22, 27–28, 33, 35–37, 68–69 |
| budgets, checked arithmetic, bounded work, failure atomicity | D-023 and D-024 | `RECOVERY_AND_LIMITS.md` §§4–9 | 81–90 |
| direct inventory, replacement order, deletion without compatibility | D-025 | `MIGRATION_AND_DELETION.md` and `IMPLEMENTATION_HANDOFF.md` | 91–100 |

## 2. Required implementation order

| Required step | Contract cut | Gate |
|---:|---|---|
| 1 | Handoff Cut 1 | existing AW-AH-009.3.1 call suite passes; no old dialogue adapter implemented |
| 2 | Handoff Cut 2 | final types/invariants compile privately without dual public API |
| 3 | Handoff Cut 3 | generic postfix/colon CST and recovery replace heuristic emission |
| 4 | Handoff Cut 4 | final source AST replaces speaker/string content model |
| 5 | Handoff Cut 5 | proof-HIR arenas/scopes/source map/poison are the sole HIR owner |
| 6 | Handoff Cut 6 | all downstream exhaustive consumers use final accessors |
| 7 | Handoff Cut 7 | all obsolete variants/helpers/tests are deleted directly |
| 8 | Handoff Cut 8 | workspace compiles and all quality gates pass before commit/merge |

The public replacement from Cut 3 through Cut 8 is one unmerged series, exactly
matching the request's direct-replacement rule.

## 3. Mandatory syntax/range coverage

| Request example | Tests |
|---|---|
| bracket/colon same meaning, distinct ranges | 1–4, 30 |
| inline/indented, blank lines, comments, Unicode, tabs, LF/CRLF | 2–9, 34–35 |
| missing `)` / `]`, empty/missing content | 10–13, 33 |
| `with:` / `with {}`, not stealing following/misaligned `with` | 14–16, 27–28, 36–37 |
| `items[0]`, expression-start collection | 17–18 |
| controls/RichText/interpolation/line breaks | 19–21 |
| exact ambiguity, no name resolution | 22, 29 |
| nested call, record argument, bare block, trivia | 23–26 |

## 4. Mandatory typed call/ID coverage

| Request example | Tests |
|---|---|
| exact target ArgumentListSyntax and all ordinary argument forms | 38–39, 50–51, 98 |
| absolute/relative/family-relative IdRef | 40–42, 80 |
| runtime expression not fabricated as ID | 43–44 |
| duplicate id/text_key retained | 45–46, 79 |
| malformed value range/recovery | 47 |

## 5. Mandatory HIR coverage

| Request example | Tests |
|---|---|
| source-backed bracket/colon ExprId and typed target links | 52–54 |
| Flow/function/closure/branch/block/statement scopes | 55–60 |
| unresolved exact candidates | 61–63 |
| whole/target/delimiter/content/plan/coordinate source lookup | 64–67 |
| recovered tooling snapshot and executable exclusion | 68–69, 92–94 |
| source revision/document identity | 70, 76–77 |
| no syntax clone/reparse/dialogue arena | 71–75, 91, 95 |
| no candidate reallocation | 78 |

## 6. Constraints and non-goals

| Constraint | Enforcement |
|---|---|
| Cut 1 runtime/domain not redesigned | D-026; tests 99; preserve matrix |
| ordinary calls/project publication/proof identity retained | D-012, D-016, preserve matrix; tests 38–39, 52–80, 98 |
| line identity/collision/text-key final acceptance deferred | D-015; coordinate records carry evidence only |
| no sema/runtime/wire/View/TTS implementation in this cut | FINAL_CONTRACT precedence and D-026 |
| no CSS/Takumi/top-level hook/memo/parser/reducer/state/borrow/`.say` route | D-026 and migration forbidden residual state |
| no compatibility/shim/dual reader/source gate/fake range/extension trait | D-020, D-025; compile/public API tests 96–97 |
| no production changes in archive | member whitelist and FINAL_STATUS verification boundary |

## 7. Expected-output trace

All 16 requested archive members are present. `OPEN_QUESTIONS.md` is exactly
`none` plus LF. `MANIFEST.txt` is sorted and has a 64-zero self-entry. External
summary, machine status, and SHA-256 sidecars are generated beside the ZIP.
