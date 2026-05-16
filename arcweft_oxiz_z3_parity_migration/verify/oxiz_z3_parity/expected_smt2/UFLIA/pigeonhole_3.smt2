; Arcweft migration fixture for UFLIA.pigeonhole_3
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/pigeonhole_3.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun pigeonhole_3_f (Int) Int)
(assert (forall ((x Int)) (= (pigeonhole_3_f x) (pigeonhole_3_f x))))
(declare-const pigeonhole_3_c Int)
(assert (not (= (pigeonhole_3_f pigeonhole_3_c) (pigeonhole_3_f pigeonhole_3_c))))
(check-sat)
