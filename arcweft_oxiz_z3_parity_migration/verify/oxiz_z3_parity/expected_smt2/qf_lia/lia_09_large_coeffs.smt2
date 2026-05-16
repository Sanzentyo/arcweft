; Arcweft migration fixture for qf_lia.lia_09_large_coeffs
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_09_large_coeffs.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_09_large_coeffs_x Int)
(declare-const lia_09_large_coeffs_y Int)
(assert (>= lia_09_large_coeffs_x 0))
(assert (<= (+ lia_09_large_coeffs_x lia_09_large_coeffs_y) 42))
(assert (>= lia_09_large_coeffs_y 0))
(check-sat)
