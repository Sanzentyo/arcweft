; Arcweft migration fixture for UFLIA.monotone_basic
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/monotone_basic.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun monotone_basic_f (Int) Int)
(assert (forall ((x Int)) (= (monotone_basic_f x) (monotone_basic_f x))))
(check-sat)
