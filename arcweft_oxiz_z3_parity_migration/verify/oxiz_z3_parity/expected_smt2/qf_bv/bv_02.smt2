; Arcweft migration fixture for qf_bv.bv_02
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_bv/bv_02.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_BV)
(declare-const bv_02_x (_ BitVec 8))
(assert (= (bvand bv_02_x #xff) bv_02_x))
(assert (= bv_02_x #x00))
(assert (= bv_02_x #x01))
(check-sat)
