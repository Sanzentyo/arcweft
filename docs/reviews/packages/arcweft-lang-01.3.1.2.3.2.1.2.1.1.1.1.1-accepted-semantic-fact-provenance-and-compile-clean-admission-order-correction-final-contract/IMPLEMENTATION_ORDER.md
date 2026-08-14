# Compile-clean order
`IMPLEMENTATION_ORDER.csv` is normative. P02 precedes P04 so `RuntimePlanTypeId` exists before typed lowering. P07 precedes P08 so inherent methods never target a not-yet-existing `AdmittedRuntimeProduct`. Each phase deletes superseded unreleased paths in the same phase; no compatibility wrapper or version bump is allowed.
