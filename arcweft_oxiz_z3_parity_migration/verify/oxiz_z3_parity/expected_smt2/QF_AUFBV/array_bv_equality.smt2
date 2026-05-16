; Arcweft migration fixture for QF_AUFBV.array_bv_equality
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_AUFBV/array_bv_equality.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_AUFBV)
(declare-fun array_bv_equality_f ((_ BitVec 8)) (_ BitVec 8))
(declare-const array_bv_equality_a (Array (_ BitVec 8) (_ BitVec 8)))
(declare-const array_bv_equality_i (_ BitVec 8))
(assert (= (array_bv_equality_f (select (store array_bv_equality_a array_bv_equality_i #x01) array_bv_equality_i)) (array_bv_equality_f #x01)))
(assert (not (= (array_bv_equality_f (select (store array_bv_equality_a array_bv_equality_i #x01) array_bv_equality_i)) (array_bv_equality_f #x01))))
(check-sat)
