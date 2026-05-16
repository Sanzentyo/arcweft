; Arcweft migration fixture for UFLIA.ackermann
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/ackermann.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun ackermann_f (Int) Int)
(assert (forall ((x Int)) (= (ackermann_f x) (ackermann_f x))))
(check-sat)
