# Requirements traceability

| ID | Request requirement | Normative decision | Completion evidence |
|---|---|---|---|
| D1 | Define accepted/rejected meanings | `FINAL_CORRECTION.md` 19.1 defines `Accepted`, `RejectedOrPoisoned`, and `CleanRecovery` from typed candidate/fact/poison/diagnostic outcomes | case-kind tests and checker/signature assertions |
| D1a | Distinguish candidate selection | family case requires a retained typed candidate before argument disposition | each fixture owns exact `CallableCandidateId` |
| D1b | Distinguish diagnostic/poison | rejected/poisoned requires a genuine typed callable diagnostic and non-clean poison | 20 negative cases |
| D1c | Distinguish clean recovery | unchecked recovery stays Selected/Clean, expected/inferred None, no callable diagnostic | three unchecked recovery tests |
| D1d | Distinguish unknown/non-callable | Missing and NonCallable are explicitly outside family evidence | non-family guard tests |
| D1e | Distinguish unsupported surface | no removed/synthetic surface can earn matrix credit | Dialogue unsupported-surface guard |
| D1f | Distinguish terminal query error | terminal errors publish no partial family fact/help | terminal guard test |
| D2 | Closed typed taxonomy | two classes and three case kinds; exactly two cases per family | 23-family classification + 46-case cardinality |
| D3 | Truthful Drop evidence | accepted + clean recovery; exact Unit result and unchecked schema | Drop schema/result/recovery tests |
| D3a | Truthful Promotion evidence | all three promotion IDs stay unchecked; representative accepted + recovery | Promotion enumeration and recovery tests |
| D3b | Truthful Speaker evidence | character/preset forms stay unchecked; accepted + recovery | Speaker enumeration and recovery tests |
| D3c | Do not manufacture rejection | unknown target/expression diagnostics cannot be relabelled callable rejection | clean recovery definition and guard tests |
| D4 | Correct section-19 quantifier | rejecting: accepted + reject/poison; unchecked: accepted + clean recovery | exact replacement section 19 |
| D4a | Resolver invocation counter | one shared resolver per target call; probes/projection do not count | dispatch audit test |
| D4b | Old dispatcher counter | zero via typed boundary observation, never source scan | old-dispatch audit test |
| D4c | Candidate parity | selected or deterministic first retained candidate parity | checker/signature parity test |
| D4d | Argument check counter | transaction-aware `TypeExpressionId` multiset count one | exactly-once test for all cases |
| D5 | Every `ALL` entry exactly once | current `ALL` order, exhaustive inherent match, set/count tests | compile gate + classification tests |
| D5a | No silent new family | exhaustive match fails compilation | inherent match |
| D5b | No silent validator-category change | schema-shape and behavioral case tests fail | drift tests |
| D6 | Reconcile parent documents | replacement has precedence over parent section 19 only | README and `FINAL_CORRECTION.md` precedence clauses |
| D6a | Preserve curried correction | Curried maps to base family; no 24th row | curried base-family test |
| D6b | Preserve typed external publication | Project fixture uses typed external binding path | Project publication test |
| T1 | Cardinality and exact-once coverage | 23 families, 46 cases, two per family | table/cardinality tests |
| T2 | Accepted + negative for rejecting schemas | 20 exact family rows | 20 second-case tests |
| T3 | Clean recovery for unchecked schemas | 3 exact family rows | Drop/Promotion/Speaker recovery tests |
| T4 | Candidate/result/count retention | exact typed outcome table | family and counter tests |
| T5 | Checker/signature primary parity | parity for every applicable case | parity test |
| T6 | No duplicate resolver/check/source scan/test semantic branch | typed audit only; one resolver; no source inspection | audit tests and structural review |
| T7 | Typed failure on family/category drift | exhaustive match + schema/case tests | compile and runtime drift gates |
| C1 | Production changes prohibited | archive contains design Markdown only | manifest/archive audit |
| C2 | No Dialogue restoration/fake Expr/second arena | current typed owner only | Dialogue row and unsupported-surface guard |
| C3 | No aliases/shims/source gates/CSS/Takumi | explicitly prohibited | implementation review and repository gates |
| C4 | Do not redesign resolver/facts/query/accounting | correction observes existing typed products; no new semantic branch | scope and ownership sections |

## Parent-row correction mapping

| Parent artifact | Original row | Corrected disposition |
|---|---|---|
| AW-AH-009.3.3 `TEST_MATRIX.md` | §19 universal “accepted and rejected call from every inventory family” | replaced in full by class-dependent two-case quantifier |
| AW-AH-009.3.3 `REQUIREMENTS_TRACEABILITY.md` | rows claiming universal accepted/rejected family evidence | read as accepted + class-required second case under this correction |
| AW-AH-009.3.3 `SURFACE_INVENTORY.md` | family inventory at older 22-family revision | current audit uses `CallableFamily::ALL` at inspected main, including `StageMethod`; semantic descriptions otherwise preserved |
| AW-AH-009.3.3.1 | Curried correction | unchanged; Curried reports base family |
| AW-AH-009.3.3.2 | external project path publication | unchanged; Project case consumes typed path |

No other parent row is superseded.
