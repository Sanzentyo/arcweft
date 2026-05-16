; Arcweft migration fixture for UFLIA.injective
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/injective.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun injective_f (Int) Int)
(assert (forall ((x Int)) (= (injective_f x) (injective_f x))))
(check-sat)
