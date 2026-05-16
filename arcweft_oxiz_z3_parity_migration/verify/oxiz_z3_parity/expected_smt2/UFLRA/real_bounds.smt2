; Arcweft migration fixture for UFLRA.real_bounds
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLRA/real_bounds.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLRA)
(declare-fun real_bounds_f (Real) Real)
(assert (forall ((x Real)) (= (real_bounds_f x) (real_bounds_f x))))
(check-sat)
