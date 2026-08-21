# Failure precedence and atomic publication

## Semantic construction precedence

1. Accepted HIR generation, owner kind, and all referenced child existence.
2. Existing type/effect/pattern/binding consistency and Bool guard.
3. Exact nominal/opaque/resource/domain owner availability.
4. Ownership recursion/cycle/limit admission.
5. Coverage work limits and unsupported pattern/domain behavior.
6. Exhaustiveness.
7. Checked semantic digest construction.
8. Unreachable warnings, sorted by source arm ordinal.

Any error in steps 1–7 suppresses Match publication and step-8 diagnostics.

## Runtime-plan/AWBC precedence

1. RuntimePlanSemanticFactInput generation and digest equality.
2. Type/producer/task-plan table bounds and semantic digest recomputation.
3. Function kind/flag constraints.
4. Opcode class and operand shape/type checks.
5. Control-flow, ownership, lifecycle and producer completeness.
6. Bundle View/AWBC join and resource digest equality.

No pending enum row is accepted without its full verifier and execution cut.

## Decode/restore precedence

1. envelope magic/version/reserved/length;
2. canonical primitive decode and allocation budgets;
3. table bounds and typed schema validation;
4. nonzero fixed IDs and transcript recomputation;
5. journal/snapshot topology and publication correlation;
6. bundle/resource/type/producer compatibility;
7. atomic runtime installation.

Failures before step 7 mutate no live state. Encoder failures truncate the
candidate Vec to its original length. Replacement failures leave the prior
active generation authoritative.
