; Arcweft migration fixture for QF_NIRA.qf_nira_real_poly
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_NIRA/qf_nira_real_poly.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_NIRA)
(declare-const qf_nira_real_poly_i Int)
(declare-const qf_nira_real_poly_r Real)
(assert (>= (* qf_nira_real_poly_i qf_nira_real_poly_i) 0))
(assert (>= (* qf_nira_real_poly_r qf_nira_real_poly_r) 0.0))
(assert (= (* qf_nira_real_poly_i qf_nira_real_poly_i) (- 1)))
(check-sat)
