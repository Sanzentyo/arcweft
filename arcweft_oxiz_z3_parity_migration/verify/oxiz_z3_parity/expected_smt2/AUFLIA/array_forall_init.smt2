; Arcweft migration fixture for AUFLIA.array_forall_init
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIA/array_forall_init.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIA)
(declare-fun array_forall_init_f (Int) Int)
(declare-const array_forall_init_a (Array Int Int))
(assert (forall ((i Int)) (= (array_forall_init_f (select array_forall_init_a i)) (array_forall_init_f (select array_forall_init_a i)))))
(check-sat)
