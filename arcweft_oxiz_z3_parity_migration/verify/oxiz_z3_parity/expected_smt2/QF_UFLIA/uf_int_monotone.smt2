; Arcweft migration fixture for QF_UFLIA.uf_int_monotone
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_UFLIA/uf_int_monotone.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_UFLIA)
(declare-fun uf_int_monotone_f (Int) Int)
(declare-const uf_int_monotone_x Int)
(assert (= (uf_int_monotone_f uf_int_monotone_x) (uf_int_monotone_f uf_int_monotone_x)))
(check-sat)
