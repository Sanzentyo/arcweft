\
# REQUIREMENTS TRACEABILITY

| Request requirement | Final decision | Contract location | Test evidence |
|---|---|---|---|
| Choose one ownership model | Select context-free ID plus existing schema-owning resolved boundary | `FINAL_CONTRACT.md` §§2, 7 | C1, RB, SR rows |
| Preserve exact two-argument constructor | Signature unchanged | §3.1 | C1-01/C1-02 |
| Decide `MissingGroup` ownership | Delete identity `MissingGroup`; zero -> `InvalidCurriedGroup`; schema absence -> existing resolver `InvalidCallGroup` | §§3.2, 3.4, 7 | C1-02, RB-02..04, RB-10 |
| Reject group zero context-free | Constructor rejects zero | §4 | C1-02, C1-05 |
| Reject recursive `Curried` wrapper | Constructor retains wrapper rejection | §4 | C1-03 |
| Reject recursive `DataLast` wrapper | Constructor retains wrapper rejection | §4 | C1-04 |
| Do not require schema in ID | Nonzero construction succeeds without schema | §§2, 4 | C1-01 |
| Reject absent/one-over project group | Existing resolved boundary returns typed resolver error | §§7, 10 | RB-02, SR-01 |
| Reject absent/one-over standard group | Same provider-neutral boundary | §§7, 10 | RB-03, SR-02 |
| Reject absent/one-over adapter group | Same provider-neutral boundary | §§7, 10 | RB-04, SR-03 |
| Positive multi-group resolution | Canonical Curried ID/instantiation and exact full schema group | §5 | RB-01, SR-04 |
| Corrupt catalogs cannot bypass | Final boundary and outcome reject, no repair/fallback | §11 | RB-09, SR-06 |
| Correct final contract prose | Self-contained normative replacement | Entire `FINAL_CONTRACT.md` | Traceability audit |
| Correct Cut 1 test matrix | Delete schema-existence row; add zero/wrapper/nonzero tests | `TEST_MATRIX.md` §A | C1-01..05 |
| Correct shared-resolver matrix | Typed provider, positive, one-over, corrupt-world rows | `TEST_MATRIX.md` §§B-C | RB/SR rows |
| Preserve implemented substrate absent flaw | Fields, signatures, schema/catalog/origin contracts retained | §§3, 13; `SURFACE_INVENTORY.md` | Regression suite |
| Add check at one successful boundary | `ResolvedCallable::try_new` only | §§5-8 | RB/SR rows |
| No second successful representation | Base ID + Curried instantiation prohibited | §§5-6 | RB-05, DR-01 |
| No second resolver/bypass | One shared route after accepted-world validation; no retry | §9 | SR-06, DR-02/03 |
| Exact implementation order | Contract -> identity correction -> resolved boundary -> provider integration -> old deletion | `IMPLEMENTATION_HANDOFF.md` | Ordered command gates |
| Delete temporary duplicate only after migration | Explicit five-condition deletion rule | §14 | SR/DR completion gate |
| No global/thread-local lookup | Explicitly rejected | §§2, 12.3, 15 | Design review |
| No schema/catalog/world in ID | ID shape unchanged | §§3.1, 15 | API/type review |
| No compatibility constructor/alias/dual reader | Direct replacement, no shim | §§3.1-3.2, 15 | Compile/test review |
| No source gate | Typed tests only | §§14-15; `TEST_MATRIX.md` rules | All tests are behavioral |
| No CSS/Takumi | Explicit non-involvement/prohibition | §15; `PRODUCTION_RECONCILIATION.md` | Change-scope review |
| OPEN_QUESTIONS=0 | All public APIs, variants, mapping, precedence, tests, and deletion rules fixed | `OPEN_QUESTIONS.md`, `FINAL_STATUS.md` | Archive validation |

## Rejected alternatives traceability

| Alternative | Decision | Reason |
|---|---|---|
| Add `&CallableSignatureSchema` to `CurriedCallableId::try_new` | Rejected | Changes fixed API, world-couples identity, duplicates resolved-boundary ownership |
| Add a separate validated curried product | Rejected | Duplicates `ResolvedCallable` and risks a competing successful representation |
| Add ambient catalog lookup | Rejected | Violates accepted-world borrowing and deterministic explicit context |
| Keep `MissingGroup` as alias/deprecated variant | Rejected | Unreleased contract correction requires direct replacement; alias would be a prohibited shim |
| Keep base-ID Curried success for migration | Rejected | It is the concrete duplicate representation defect; old route may coexist only before family migration, not as a second product |
