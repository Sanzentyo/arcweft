# Package validator

Run from the package root or any working directory:

```text
python3 tools/validate_package.py --package-root .
python3 tools/validate_package.py --package-root . --self-test
```

The validator is read-only with respect to the package. Negative self-tests use
a temporary copy of the machine JSON and do not mutate source artifacts.

## Positive checks

The validator closes these classes:

- package identity, inspected SHA, `READY_FOR_IMPLEMENTATION`, and
  `OPEN_QUESTIONS=0`;
- every Arcweft-owned version marker equals `1`;
- exact `TaskSpec`/`TaskExecution` shape and absence of `AdapterCommit`;
- exactly nine producer-family routes with allowed execution/policy values;
- the complete 72-row persistence graph, strict decoder declarations, and
  required snapshot/replay/replacement owners;
- the exact Match expression/value/select/pattern/literal inventories;
- the exact 85-row `TypeKind` ownership matrix, including Predicate leaf,
  Shared rejection, and four-part evidence on every `SnapshotClone`;
- compiler-local versus persistent View row separation;
- exact five-cut dependency order, including no cut-3 dependency on cut-4
  task types and no cut-4 public RuntimeValue variant;
- current-path deletion rows, test traceability, required prose/CSV artifacts,
  and manifest hashes.

## Negative self-tests

Each named mutation must fail:

1. `adapter_commit`
2. `unconditional_host_request`
3. `undefined_snapshot`
4. `compiler_local_bundle_id`
5. `shared_without_carrier`
6. `predicate_recursion`
7. `private_runtime_value_variant`
8. `cut3_depends_on_cut4`
9. `missing_producer_family`
10. `version_two`

A negative test is itself a failure when the corrupted package is accepted or
when rejection occurs only because the mutation harness is malformed.

## Exit codes

- `0`: all requested positive checks and optional negative self-tests pass.
- `1`: one or more contract violations.
- `2`: command-line or package-root error.

Diagnostics are stable, sorted, and path-relative so the checked output is
usable in review automation.
