; Arcweft migration fixture for QF_NIRA.qf_nira_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_NIRA/qf_nira_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_NIRA)
(declare-const qf_nira_unsat_i Int)
(declare-const qf_nira_unsat_r Real)
(assert (>= (* qf_nira_unsat_i qf_nira_unsat_i) 0))
(assert (>= (* qf_nira_unsat_r qf_nira_unsat_r) 0.0))
(assert (= (* qf_nira_unsat_i qf_nira_unsat_i) (- 1)))
(check-sat)
