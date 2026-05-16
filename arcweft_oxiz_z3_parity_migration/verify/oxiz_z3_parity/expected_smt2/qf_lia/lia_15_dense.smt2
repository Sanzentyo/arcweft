; Arcweft migration fixture for qf_lia.lia_15_dense
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_15_dense.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_15_dense_x Int)
(declare-const lia_15_dense_y Int)
(assert (>= lia_15_dense_x 0))
(assert (<= (+ lia_15_dense_x lia_15_dense_y) 42))
(assert (>= lia_15_dense_y 0))
(check-sat)
