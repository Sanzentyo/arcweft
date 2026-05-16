; Arcweft migration fixture for QF_UFLRA.uf_real_linear
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_UFLRA/uf_real_linear.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_UFLRA)
(declare-fun uf_real_linear_f (Real) Real)
(declare-const uf_real_linear_x Real)
(assert (= (uf_real_linear_f uf_real_linear_x) (uf_real_linear_f uf_real_linear_x)))
(check-sat)
