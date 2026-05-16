; Arcweft migration fixture for qf_lia.lia_13_divisibility
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_lia/lia_13_divisibility.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_LIA)
(declare-const lia_13_divisibility_x Int)
(declare-const lia_13_divisibility_y Int)
(assert (>= lia_13_divisibility_x 0))
(assert (<= (+ lia_13_divisibility_x lia_13_divisibility_y) 42))
(assert (>= lia_13_divisibility_y 0))
(check-sat)
