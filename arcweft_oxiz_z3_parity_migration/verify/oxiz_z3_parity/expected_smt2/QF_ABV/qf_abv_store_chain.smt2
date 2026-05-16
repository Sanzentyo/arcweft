; Arcweft migration fixture for QF_ABV.qf_abv_store_chain
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/QF_ABV/qf_abv_store_chain.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_ABV)
(declare-const qf_abv_store_chain_a (Array (_ BitVec 8) (_ BitVec 8)))
(declare-const qf_abv_store_chain_i (_ BitVec 8))
(assert (= (select (store qf_abv_store_chain_a qf_abv_store_chain_i #x2a) qf_abv_store_chain_i) #x2a))
(check-sat)
