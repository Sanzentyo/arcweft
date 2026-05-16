; Arcweft migration fixture for UFLIA.forall_exists_simple
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/forall_exists_simple.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun forall_exists_simple_f (Int) Int)
(assert (forall ((x Int)) (= (forall_exists_simple_f x) (forall_exists_simple_f x))))
(check-sat)
