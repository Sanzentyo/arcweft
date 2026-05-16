; Arcweft migration fixture for qf_s.string_08
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_s/string_08.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_S)
(declare-const string_08_s String)
(assert (= (str.++ string_08_s "") string_08_s))
(assert (= string_08_s "a"))
(assert (= string_08_s "b"))
(check-sat)
