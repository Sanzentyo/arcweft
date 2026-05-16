; Arcweft migration fixture for QF_UFLRA.uf_real_interp
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_UFLRA/uf_real_interp.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_UFLRA)
(declare-fun uf_real_interp_f (Real) Real)
(declare-const uf_real_interp_x Real)
(assert (= (uf_real_interp_f uf_real_interp_x) (uf_real_interp_f uf_real_interp_x)))
(assert (not (= (uf_real_interp_f uf_real_interp_x) (uf_real_interp_f uf_real_interp_x))))
(check-sat)
