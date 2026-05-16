; Arcweft migration fixture for qf_dt.dt_01
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_01.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_01_Box 0)) (((dt_01_box (dt_01_val Int)))))
(declare-const dt_01_b dt_01_Box)
(assert (= dt_01_b dt_01_b))
(check-sat)
