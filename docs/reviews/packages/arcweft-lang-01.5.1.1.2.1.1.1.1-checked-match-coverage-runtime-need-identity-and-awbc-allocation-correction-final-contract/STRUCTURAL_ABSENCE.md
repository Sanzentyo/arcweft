# Structural absence rules

The final implementation and generated artifacts must contain none of the
following production routes:

- caller-supplied `CheckedMatchCoverage`, exhaustiveness bool, or unreachable
  list;
- String-backed `NeedId`, `TaskKey`, or `TaskId`, arbitrary parser, indexed
  suffix, or display-derived identity;
- payloadless, String, or Dynamic NeedHandle carrier;
- optional legacy `AwbcTaskPlan.need_id` or fallback from TaskId;
- feature-local numeric opcode/function-kind/flag table, raw opcode DTO, String
  opcode tag, unsafe/transmute decoder, or duplicate reader;
- ordinary fixed-LE u32, `usize::Wire`, tensor-shape asymmetry, or payload
  temporary buffer;
- compatibility reader, alias, serde alias, dual carrier, fallback resolver,
  source reconstruction, or old snapshot default;
- persisted `ExprId`, `ScopeId`, `PatternId`, `LocalId`, HIR snapshot/arena
  coordinate, debug formatting, source spelling, or platform-sized integer;
- copied nominal/resource/endpoint type table;
- View VM, multi-result AWBC, retained callee register/frame export, or
  `arcweft-view` dependency on core;
- nonexistent `RuntimeSemanticFactInput` or nominal `AwbcVm` owner;
- extension trait or ad hoc helper used to avoid adding behavior to an
  Arcweft-owned enum's inherent implementation;
- pending opcode enum variant without full verifier, VM, structured, AOT,
  snapshot and test support.

`machine/structural-absence.json` is the validator-readable closed list. Source
implementation acceptance must use compiler/type/build checks and targeted AST
or rustdoc queries rather than brittle repository-wide text scans.
