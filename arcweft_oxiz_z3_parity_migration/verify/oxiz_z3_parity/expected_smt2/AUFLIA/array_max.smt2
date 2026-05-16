; Arcweft migration fixture for AUFLIA.array_max
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIA/array_max.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIA)
(declare-fun array_max_f (Int) Int)
(declare-const array_max_a (Array Int Int))
(assert (forall ((i Int)) (= (array_max_f (select array_max_a i)) (array_max_f (select array_max_a i)))))
(check-sat)
