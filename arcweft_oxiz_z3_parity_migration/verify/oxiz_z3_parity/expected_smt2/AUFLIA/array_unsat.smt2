; Arcweft migration fixture for AUFLIA.array_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIA/array_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIA)
(declare-fun array_unsat_f (Int) Int)
(declare-const array_unsat_a (Array Int Int))
(assert (forall ((i Int)) (= (array_unsat_f (select array_unsat_a i)) (array_unsat_f (select array_unsat_a i)))))
(declare-const array_unsat_k Int)
(assert (not (= (array_unsat_f (select array_unsat_a array_unsat_k)) (array_unsat_f (select array_unsat_a array_unsat_k)))))
(check-sat)
