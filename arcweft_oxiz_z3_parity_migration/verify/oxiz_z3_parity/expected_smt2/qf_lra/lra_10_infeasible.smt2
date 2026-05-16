; Arcweft migration fixture for qf_lra.lra_10_infeasible
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_10_infeasible.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_10_infeasible_x Real)
(declare-const lra_10_infeasible_y Real)
(assert (<= (+ lra_10_infeasible_x (* 2.0 lra_10_infeasible_y)) 9.0))
(assert (= lra_10_infeasible_x 0.0))
(assert (= lra_10_infeasible_x 1.0))
(check-sat)
