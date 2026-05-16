; Arcweft migration fixture for UFLIA.trans_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/UFLIA/trans_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic UFLIA)
(declare-fun trans_unsat_f (Int) Int)
(assert (forall ((x Int)) (= (trans_unsat_f x) (trans_unsat_f x))))
(declare-const trans_unsat_c Int)
(assert (not (= (trans_unsat_f trans_unsat_c) (trans_unsat_f trans_unsat_c))))
(check-sat)
