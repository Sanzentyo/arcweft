; Arcweft migration fixture for qf_lia.lia_02_linear_eq
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_02_linear_eq.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_02_linear_eq_x Int)
(declare-const lia_02_linear_eq_y Int)
(assert (>= lia_02_linear_eq_x 0))
(assert (<= (+ lia_02_linear_eq_x lia_02_linear_eq_y) 42))
(assert (>= lia_02_linear_eq_y 0))
(check-sat)
