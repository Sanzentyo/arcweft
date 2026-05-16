; Arcweft migration fixture for AUFLIRA.auflira_basic_sat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIRA/auflira_basic_sat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIRA)
(declare-fun auflira_basic_sat_f (Real) Real)
(declare-const auflira_basic_sat_a (Array Int Real))
(assert (forall ((i Int)) (= (auflira_basic_sat_f (select auflira_basic_sat_a i)) (auflira_basic_sat_f (select auflira_basic_sat_a i)))))
(check-sat)
