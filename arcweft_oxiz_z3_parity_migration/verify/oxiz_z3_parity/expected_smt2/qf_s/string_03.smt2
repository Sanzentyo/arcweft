; Arcweft migration fixture for qf_s.string_03
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_s/string_03.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_S)
(declare-const string_03_s String)
(assert (= (str.++ string_03_s "") string_03_s))
(check-sat)
