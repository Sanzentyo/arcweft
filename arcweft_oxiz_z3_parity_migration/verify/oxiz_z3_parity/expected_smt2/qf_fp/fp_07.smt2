; Arcweft migration fixture for qf_fp.fp_07
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_fp/fp_07.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_FP)
(declare-const fp_07_x (_ FloatingPoint 8 24))
(assert (= fp_07_x fp_07_x))
(check-sat)
