; Arcweft migration fixture for UFLIA.skolem_test
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/skolem_test.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun skolem_test_f (Int) Int)
(assert (forall ((x Int)) (= (skolem_test_f x) (skolem_test_f x))))
(check-sat)
