; Arcweft migration fixture for QF_ABV.qf_abv_contradiction
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_ABV/qf_abv_contradiction.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_ABV)
(declare-const qf_abv_contradiction_a (Array (_ BitVec 8) (_ BitVec 8)))
(declare-const qf_abv_contradiction_i (_ BitVec 8))
(assert (= (select (store qf_abv_contradiction_a qf_abv_contradiction_i #x2a) qf_abv_contradiction_i) #x2a))
(assert (= (select (store qf_abv_contradiction_a qf_abv_contradiction_i #x2a) qf_abv_contradiction_i) #x00))
(check-sat)
