; Arcweft migration fixture for UFLIA.congruence_closure
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/congruence_closure.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun congruence_closure_f (Int) Int)
(assert (forall ((x Int)) (= (congruence_closure_f x) (congruence_closure_f x))))
(check-sat)
