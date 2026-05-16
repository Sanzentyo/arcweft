; Arcweft migration fixture for AUFLIRA.auflira_unsat
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIRA/auflira_unsat.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIRA)
(declare-fun auflira_unsat_f (Real) Real)
(declare-const auflira_unsat_a (Array Int Real))
(assert (forall ((i Int)) (= (auflira_unsat_f (select auflira_unsat_a i)) (auflira_unsat_f (select auflira_unsat_a i)))))
(declare-const auflira_unsat_k Int)
(assert (not (= (auflira_unsat_f (select auflira_unsat_a auflira_unsat_k)) (auflira_unsat_f (select auflira_unsat_a auflira_unsat_k)))))
(check-sat)
