; Arcweft migration fixture for qf_s.string_02
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_s/string_02.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_S)
(declare-const string_02_s String)
(assert (= (str.++ string_02_s "") string_02_s))
(assert (= string_02_s "a"))
(assert (= string_02_s "b"))
(check-sat)
