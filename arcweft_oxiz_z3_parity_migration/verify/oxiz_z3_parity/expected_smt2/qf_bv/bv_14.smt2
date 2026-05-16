; Arcweft migration fixture for qf_bv.bv_14
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_bv/bv_14.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_BV)
(declare-const bv_14_x (_ BitVec 8))
(assert (= (bvand bv_14_x #xff) bv_14_x))
(check-sat)
