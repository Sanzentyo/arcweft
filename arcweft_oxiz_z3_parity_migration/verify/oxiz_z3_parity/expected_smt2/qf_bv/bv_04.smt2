; Arcweft migration fixture for qf_bv.bv_04
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_bv/bv_04.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_BV)
(declare-const bv_04_x (_ BitVec 8))
(assert (= (bvand bv_04_x #xff) bv_04_x))
(check-sat)
