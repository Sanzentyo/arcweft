; Arcweft migration fixture for QF_AUFBV.array_bv_ite_select
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_AUFBV/array_bv_ite_select.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_AUFBV)
(declare-fun array_bv_ite_select_f ((_ BitVec 8)) (_ BitVec 8))
(declare-const array_bv_ite_select_a (Array (_ BitVec 8) (_ BitVec 8)))
(declare-const array_bv_ite_select_i (_ BitVec 8))
(assert (= (array_bv_ite_select_f (select (store array_bv_ite_select_a array_bv_ite_select_i #x01) array_bv_ite_select_i)) (array_bv_ite_select_f #x01)))
(check-sat)
