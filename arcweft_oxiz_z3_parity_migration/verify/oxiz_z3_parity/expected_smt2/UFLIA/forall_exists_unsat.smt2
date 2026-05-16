; Arcweft migration fixture for UFLIA.forall_exists_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/forall_exists_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun forall_exists_unsat_f (Int) Int)
(assert (forall ((x Int)) (= (forall_exists_unsat_f x) (forall_exists_unsat_f x))))
(declare-const forall_exists_unsat_c Int)
(assert (not (= (forall_exists_unsat_f forall_exists_unsat_c) (forall_exists_unsat_f forall_exists_unsat_c))))
(check-sat)
