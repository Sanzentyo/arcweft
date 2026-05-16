; Arcweft migration fixture for UFLIA.nested_quantifiers
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/nested_quantifiers.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun nested_quantifiers_f (Int) Int)
(assert (forall ((x Int)) (= (nested_quantifiers_f x) (nested_quantifiers_f x))))
(check-sat)
