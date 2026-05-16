; Arcweft migration fixture for UFLIA.linear_order
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/linear_order.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun linear_order_f (Int) Int)
(assert (forall ((x Int)) (= (linear_order_f x) (linear_order_f x))))
(check-sat)
