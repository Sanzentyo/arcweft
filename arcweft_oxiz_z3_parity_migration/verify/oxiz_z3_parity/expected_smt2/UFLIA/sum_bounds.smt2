; Arcweft migration fixture for UFLIA.sum_bounds
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/sum_bounds.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun sum_bounds_f (Int) Int)
(assert (forall ((x Int)) (= (sum_bounds_f x) (sum_bounds_f x))))
(check-sat)
