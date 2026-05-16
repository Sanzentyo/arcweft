; Arcweft migration fixture for AUFLIRA.auflira_quantified
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIRA/auflira_quantified.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIRA)
(declare-fun auflira_quantified_f (Real) Real)
(declare-const auflira_quantified_a (Array Int Real))
(assert (forall ((i Int)) (= (auflira_quantified_f (select auflira_quantified_a i)) (auflira_quantified_f (select auflira_quantified_a i)))))
(declare-const auflira_quantified_k Int)
(assert (not (= (auflira_quantified_f (select auflira_quantified_a auflira_quantified_k)) (auflira_quantified_f (select auflira_quantified_a auflira_quantified_k)))))
(check-sat)
