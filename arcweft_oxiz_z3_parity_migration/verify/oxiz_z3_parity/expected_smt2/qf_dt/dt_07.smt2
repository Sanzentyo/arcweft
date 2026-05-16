; Arcweft migration fixture for qf_dt.dt_07
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_07.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_07_Box 0)) (((dt_07_box (dt_07_val Int)))))
(declare-const dt_07_b dt_07_Box)
(assert (= dt_07_b dt_07_b))
(check-sat)
