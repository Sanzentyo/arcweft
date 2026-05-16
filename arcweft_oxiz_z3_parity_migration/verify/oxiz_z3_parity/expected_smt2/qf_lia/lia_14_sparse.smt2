; Arcweft migration fixture for qf_lia.lia_14_sparse
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_14_sparse.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_14_sparse_x Int)
(declare-const lia_14_sparse_y Int)
(assert (>= lia_14_sparse_x 0))
(assert (<= (+ lia_14_sparse_x lia_14_sparse_y) 42))
(assert (>= lia_14_sparse_y 0))
(check-sat)
