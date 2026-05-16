; Arcweft migration fixture for qf_lra.lra_05_unsat_simple
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_05_unsat_simple.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_05_unsat_simple_x Real)
(declare-const lra_05_unsat_simple_y Real)
(assert (<= (+ lra_05_unsat_simple_x (* 2.0 lra_05_unsat_simple_y)) 9.0))
(assert (= lra_05_unsat_simple_x 0.0))
(assert (= lra_05_unsat_simple_x 1.0))
(check-sat)
