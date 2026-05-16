; Arcweft migration fixture for UFLIA.injective_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/injective_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun injective_unsat_f (Int) Int)
(assert (forall ((x Int)) (= (injective_unsat_f x) (injective_unsat_f x))))
(declare-const injective_unsat_c Int)
(assert (not (= (injective_unsat_f injective_unsat_c) (injective_unsat_f injective_unsat_c))))
(check-sat)
