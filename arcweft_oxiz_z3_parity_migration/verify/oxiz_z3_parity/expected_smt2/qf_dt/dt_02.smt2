; Arcweft migration fixture for qf_dt.dt_02
; Source: cool-japan/oxiz:bench/z3_parity/benchmarks/qf_dt/dt_02.smt2@9f6bb93df338fd8e965511e9e1abc97ed3ca395f
; Expected: sat
; Equivalence scope: normalized logic/satisfiability fixture for Arcweft SMT emission tests.

(set-logic QF_DT)
(declare-datatypes ((dt_02_Box 0)) (((dt_02_box (dt_02_val Int)))))
(declare-const dt_02_b dt_02_Box)
(assert (= dt_02_b dt_02_b))
(check-sat)
