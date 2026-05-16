; Arcweft migration fixture for qf_fp.fp_10
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_fp/fp_10.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: unsat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_FP)
(declare-const fp_10_x (_ FloatingPoint 8 24))
(assert (= fp_10_x fp_10_x))
(assert (not (= fp_10_x fp_10_x)))
(check-sat)
