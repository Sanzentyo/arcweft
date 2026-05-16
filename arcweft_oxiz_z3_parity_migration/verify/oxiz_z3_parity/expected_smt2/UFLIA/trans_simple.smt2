; Arcweft migration fixture for UFLIA.trans_simple
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/trans_simple.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun trans_simple_f (Int) Int)
(assert (forall ((x Int)) (= (trans_simple_f x) (trans_simple_f x))))
(check-sat)
