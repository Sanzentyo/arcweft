; Arcweft migration fixture for AUFLIA.array_search
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIA/array_search.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIA)
(declare-fun array_search_f (Int) Int)
(declare-const array_search_a (Array Int Int))
(assert (forall ((i Int)) (= (array_search_f (select array_search_a i)) (array_search_f (select array_search_a i)))))
(check-sat)
