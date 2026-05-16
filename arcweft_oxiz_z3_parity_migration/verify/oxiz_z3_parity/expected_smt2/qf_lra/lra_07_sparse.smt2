; Arcweft migration fixture for qf_lra.lra_07_sparse
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_07_sparse.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_07_sparse_x Real)
(declare-const lra_07_sparse_y Real)
(assert (<= (+ lra_07_sparse_x (* 2.0 lra_07_sparse_y)) 9.0))
(assert (> lra_07_sparse_x (- 1.0)))
(check-sat)
