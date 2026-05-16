; Arcweft migration fixture for qf_bv.bv_06
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_bv/bv_06.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_BV)
(declare-const bv_06_x (_ BitVec 8))
(assert (= (bvand bv_06_x #xff) bv_06_x))
(assert (= bv_06_x #x00))
(assert (= bv_06_x #x01))
(check-sat)
