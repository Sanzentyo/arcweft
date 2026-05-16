; Arcweft migration fixture for UFLIA.division_property
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/division_property.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun division_property_f (Int) Int)
(assert (forall ((x Int)) (= (division_property_f x) (division_property_f x))))
(check-sat)
