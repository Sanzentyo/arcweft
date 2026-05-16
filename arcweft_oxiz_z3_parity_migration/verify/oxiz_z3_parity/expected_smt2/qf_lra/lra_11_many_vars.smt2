; Arcweft migration fixture for qf_lra.lra_11_many_vars
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_11_many_vars.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_11_many_vars_x Real)
(declare-const lra_11_many_vars_y Real)
(assert (<= (+ lra_11_many_vars_x (* 2.0 lra_11_many_vars_y)) 9.0))
(assert (> lra_11_many_vars_x (- 1.0)))
(check-sat)
