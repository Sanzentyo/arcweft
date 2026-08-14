# Decision 08 — producer/consumer/deletion inventory and tests

`PRODUCER_CONSUMER_DELETION_INVENTORY.csv` is the normative migration inventory. It retains unchanged parent rows, corrects every invalid retry row, and adds explicit child rows for:

- the sole value path owner and all affine/checked consumers;
- `RuntimeCheckedTypePath` and structured errors;
- every mandatory plan type field/wrapper/path/site;
- every AWBC typed owner, slot enum, origin, reference resolver, and deleted root-use row;
- independent generation facts and pair equality;
- validator/context phase order;
- complete outer shapes and nominal descriptor evidence;
- private wire DTOs/custom deserialization;
- dialogue catalog bridge and current-world role registry.

`TEST_MATRIX.csv` is normative. It includes positive, negative, round-trip, property, compile-fail, codec/golden, integration, tamper, static-deletion, and phase-build cases. It explicitly tests every row of the path, expression, pattern, plan-site, AWBC-site, instruction-slot, terminator-slot, audio-slot, and reference-resolution tables.

A concept removed by this correction is not retained as a compatibility test target. Tests named for `RuntimePlanTypedRootUse`, `AwbcTypedRootUse`, a pattern-owned `RuntimeValuePath`, runtime-driver-owned `AdmittedGenerationCatalogs`, `target_generation`, `MissingCharacterView`, or `AcceptedNominalEnvironment` are replaced by static-absence tests and final-owner tests.
