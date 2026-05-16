; Arcweft migration fixture for qf_lra.lra_09_tight_bounds
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_09_tight_bounds.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_09_tight_bounds_x Real)
(declare-const lra_09_tight_bounds_y Real)
(assert (<= (+ lra_09_tight_bounds_x (* 2.0 lra_09_tight_bounds_y)) 9.0))
(assert (> lra_09_tight_bounds_x (- 1.0)))
(check-sat)
