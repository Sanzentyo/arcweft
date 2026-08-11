# Requirements traceability

## 1. Required decisions

| Request decision | Selected answer | Normative owner | Direct tests |
|---|---|---|---|
| 1. One variant, distinct variants, or generalized typed surface | One semantic `Expr::Call(CallExpr)` with exhaustive `CallSurfaceSyntax::{Parenthesized, CallbackBlock}` | `FINAL_CONTRACT.md` §§2, 12 | P-01–P-06, C-01–C-05, M-01 |
| 2. Parenthesized carrier and callback exact ranges | `ParenthesizedCallSyntax` owns non-optional `ArgumentListSyntax`; `CallbackBlockCallSyntax` owns exact `CallbackBlockSyntax` | `FINAL_CONTRACT.md` §§3–4 | P-01–P-06, C-01–C-05 |
| 3. Callback signature-help behavior | Outer callback surface is always `NotApplicable`; nested parenthesized calls remain eligible | `FINAL_CONTRACT.md` §8 | S-04–S-06 |
| 4. Parser-only source construction and generated expressions | Delete public call constructors; source tests parse; generated executable calls use existing `RuntimeExpr::Call` | `FINAL_CONTRACT.md` §§5, 10 | G-01–G-03 |
| 5. Missing `)` recovery and owner boundary | `RecoveredMissing { insertion, boundary }`, stop before owner token, retain typed call and diagnostic | `FINAL_CONTRACT.md` §6 | R-01–R-05, H-02, S-03 |
| 6. HIR clone and one sema resolver | New syntax types derive `Clone`; current HIR retains syntax Expr; ordinary sema uses accessors; one resolver projects only parenthesized lists | `FINAL_CONTRACT.md` §§8–9 | H-01–H-04, S-01–S-06, M-01–M-02 |
| 7. Exact direct migration without compatibility | Old variant/constructors deleted; Cuts 2–4 are one unmerged direct-replacement series; compiler exhaustiveness migrates all consumers | `IMPLEMENTATION_HANDOFF.md` §§3–5 | G-01, M-02, workspace gates |

## 2. Required implementation order

| Required order | Contract cut | Completion gate |
|---|---|---|
| 1. Syntax-owned range types and validation | `IMPLEMENTATION_HANDOFF.md` §2 | syntax check/Clippy/unit tests |
| 2. Pratt, callback, dialogue/speaker, recovery | §3 | syntax package check/Clippy/all targets |
| 3. Remove source-less construction and direct callers | §4 | constructor-dependent package frontier |
| 4. Exhaustive syntax/HIR/sema/tooling replacement | §5 | workspace check and Clippy |
| 5. Resume AW-AH-009.3 cuts 1–6 | §6 | parent focused tests using only parenthesized carrier |
| 6. Focused/workspace/structural validation | §7 | focused tests, workspace check/Clippy/test, format, diff, canonical audit |

## 3. Mandatory direct tests

| Mandatory coverage | Test matrix coverage |
|---|---|
| Empty parenthesized call | P-01 |
| Positional and exact UTF-8 ranges | P-02 |
| Named | P-03 |
| Spread | P-04 |
| Trailing comma | P-05 |
| Nested parenthesized calls | P-06 |
| Missing `)` exact recovery | R-01, R-02 |
| Isolated malformed arguments | R-03, R-04 |
| Callback zero parameters | C-01 |
| Callback one parameter | C-02 |
| Callback multiple parameters | C-03 |
| Callback typed parameters | C-04 |
| Callback multi-statement body | C-05 |
| Parenthesized call followed by selected callback | C-05, S-06 |
| Generated/programmatic non-authored expressions | G-02 |
| HIR preservation of each surface | H-01–H-04 |
| Signature help inside parentheses | S-01–S-03 |
| Explicit callback-brace behavior | S-04–S-06 |
| Current-grammar malformed rejection | R-05, C-06, M-03 |
| No removed spelling recognizer/source scan | M-03, §11 of `TEST_MATRIX.md` |

## 4. Fixed constraints and non-goals

| Constraint | Contract enforcement |
|---|---|
| Do not redesign `CharacterNominalType` | `PRODUCTION_RECONCILIATION.md` §7; inherited unchanged |
| Do not redesign accepted semantic-world publication | Same section; Cut 5 consumes parent contract |
| Do not redesign checked LSP position conversion | Same section |
| Do not redesign parent resolver/cache policies | `FINAL_CONTRACT.md` §§1, 8, 11 |
| No legacy call variant or compatibility constructor | `FINAL_CONTRACT.md` §12; `IMPLEMENTATION_HANDOFF.md` §§1, 4 |
| No deprecated accessor or extension trait | `FINAL_CONTRACT.md` §§5, 12 |
| No dual AST | One `CallExpr`, HIR clones syntax; §§2, 9 |
| No stringly delimiter kind | Typed surface enum and `CallRecoveryTokenKind`; §§2–4 |
| No fake range | Closed versus `RecoveredMissing`; callback close required; §§3–6 |
| No source gates | `IMPLEMENTATION_HANDOFF.md` §7.4; `TEST_MATRIX.md` §11 |
| No after-the-fact source search | Parser token ownership and helper deletions; `FINAL_CONTRACT.md` §6.4 |
| No signature-specific node identity | Existing document/HIR path only; §§8–9 |
| No CSS/Takumi or removed borrow-block restoration | `PRODUCTION_RECONCILIATION.md` §7 |
| Proof concurrency not a prerequisite | `PRODUCTION_RECONCILIATION.md` §§2, 7 |

## 5. Expected archive and status

| Output requirement | Satisfaction |
|---|---|
| Exact ZIP name | `arcweft-aw-ah-009.3.1-call-surface-syntax-production-reconciliation-final-contract.zip` |
| Required members only | Verified by package audit and `MANIFEST.txt` |
| `OPEN_QUESTIONS.md` exactly `none` | Four ASCII bytes, no newline |
| Sorted manifest | Lexicographically sorted relative paths |
| Manifest self-entry | 64 ASCII zeroes |
| Summary/status/SHA sidecars | Returned next to ZIP |
| One final model | `FINAL_CONTRACT.md` §§2–4 |
| Zero result-changing decisions | `OPEN_QUESTIONS.md`, `FINAL_STATUS.md` |
| Complete migration | `IMPLEMENTATION_HANDOFF.md` §§2–7 |
| No fabricated range | All ranges authored or exact recovery insertion/boundary |

## 6. Readiness gate

Every result-changing decision is mapped above to one normative section and at least one observable test. The implementation order names the owner, deletion point, package frontier, and final workspace gate. No requirement is deferred to the implementation assignee.
