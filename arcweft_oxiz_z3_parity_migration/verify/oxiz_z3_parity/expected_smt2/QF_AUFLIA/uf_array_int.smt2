; Arcweft migration fixture for QF_AUFLIA.uf_array_int
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_AUFLIA/uf_array_int.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_AUFLIA)
(declare-fun uf_array_int_f (Int) Int)
(declare-const uf_array_int_a (Array Int Int))
(declare-const uf_array_int_i Int)
(assert (= (uf_array_int_f (select (store uf_array_int_a uf_array_int_i 5) uf_array_int_i)) (uf_array_int_f 5)))
(check-sat)
