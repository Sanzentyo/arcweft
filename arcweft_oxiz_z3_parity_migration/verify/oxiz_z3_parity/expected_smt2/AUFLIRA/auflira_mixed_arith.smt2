; Arcweft migration fixture for AUFLIRA.auflira_mixed_arith
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/AUFLIRA/auflira_mixed_arith.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic AUFLIRA)
(declare-fun auflira_mixed_arith_f (Real) Real)
(declare-const auflira_mixed_arith_a (Array Int Real))
(assert (forall ((i Int)) (= (auflira_mixed_arith_f (select auflira_mixed_arith_a i)) (auflira_mixed_arith_f (select auflira_mixed_arith_a i)))))
(declare-const auflira_mixed_arith_k Int)
(assert (not (= (auflira_mixed_arith_f (select auflira_mixed_arith_a auflira_mixed_arith_k)) (auflira_mixed_arith_f (select auflira_mixed_arith_a auflira_mixed_arith_k)))))
(check-sat)
