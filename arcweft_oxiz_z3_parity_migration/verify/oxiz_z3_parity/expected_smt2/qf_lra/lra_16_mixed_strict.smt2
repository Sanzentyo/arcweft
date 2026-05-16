; Arcweft migration fixture for qf_lra.lra_16_mixed_strict
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lra/lra_16_mixed_strict.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LRA)
(declare-const lra_16_mixed_strict_x Real)
(declare-const lra_16_mixed_strict_y Real)
(assert (<= (+ lra_16_mixed_strict_x (* 2.0 lra_16_mixed_strict_y)) 9.0))
(assert (> lra_16_mixed_strict_x (- 1.0)))
(check-sat)
