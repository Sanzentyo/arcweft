; Arcweft migration fixture for QF_UFLIA.uf_int_basic
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_UFLIA/uf_int_basic.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_UFLIA)
(declare-fun uf_int_basic_f (Int) Int)
(declare-const uf_int_basic_x Int)
(assert (= (uf_int_basic_f uf_int_basic_x) (uf_int_basic_f uf_int_basic_x)))
(check-sat)
