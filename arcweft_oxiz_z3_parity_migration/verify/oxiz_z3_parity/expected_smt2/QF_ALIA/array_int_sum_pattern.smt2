; Arcweft migration fixture for QF_ALIA.array_int_sum_pattern
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_ALIA/array_int_sum_pattern.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_ALIA)
(declare-const array_int_sum_pattern_a (Array Int Int))
(declare-const array_int_sum_pattern_i Int)
(assert (= (select (store array_int_sum_pattern_a array_int_sum_pattern_i 7) array_int_sum_pattern_i) 7))
(check-sat)
