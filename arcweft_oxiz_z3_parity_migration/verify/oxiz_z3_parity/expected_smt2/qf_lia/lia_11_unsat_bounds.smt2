; Arcweft migration fixture for qf_lia.lia_11_unsat_bounds
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_11_unsat_bounds.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_11_unsat_bounds_x Int)
(declare-const lia_11_unsat_bounds_y Int)
(assert (>= lia_11_unsat_bounds_x 0))
(assert (<= (+ lia_11_unsat_bounds_x lia_11_unsat_bounds_y) 42))
(assert (= lia_11_unsat_bounds_x 0))
(assert (= lia_11_unsat_bounds_x 1))
(check-sat)
