; Arcweft migration fixture for UFLIA.idempotent
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/idempotent.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun idempotent_f (Int) Int)
(assert (forall ((x Int)) (= (idempotent_f x) (idempotent_f x))))
(check-sat)
