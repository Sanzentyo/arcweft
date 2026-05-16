; Arcweft migration fixture for UFLIA.arith_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/arith_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun arith_unsat_f (Int) Int)
(assert (forall ((x Int)) (= (arith_unsat_f x) (arith_unsat_f x))))
(declare-const arith_unsat_c Int)
(assert (not (= (arith_unsat_f arith_unsat_c) (arith_unsat_f arith_unsat_c))))
(check-sat)
