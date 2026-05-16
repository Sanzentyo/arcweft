; Arcweft migration fixture for qf_lia.lia_07_mixed_eq_ineq
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_07_mixed_eq_ineq.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_07_mixed_eq_ineq_x Int)
(declare-const lia_07_mixed_eq_ineq_y Int)
(assert (>= lia_07_mixed_eq_ineq_x 0))
(assert (<= (+ lia_07_mixed_eq_ineq_x lia_07_mixed_eq_ineq_y) 42))
(assert (>= lia_07_mixed_eq_ineq_y 0))
(check-sat)
