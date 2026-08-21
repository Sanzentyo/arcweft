# Failed returned archive evidence

Request-supplied archive SHA-256:
`C5857AFCFCDDC88D2F642C4B4ACB0E61A68BBC4AC0BE42755BA9C2593B20E732`.

Repository-retained validator result:

```json
{
  "pass": false,
  "issues": [
    "MISSING README.md",
    "MISSING 2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-concrete-design.md",
    "MISSING REQUIREMENT-TRACEABILITY.md",
    "MISSING SOURCE-EVIDENCE.md",
    "MISSING TEST-MATRIX.md",
    "MISSING IMPLEMENTATION-SEQUENCE.md",
    "MISSING VERIFICATION.md",
    "INSUFFICIENT_RUST_LINE_EVIDENCE",
    "TRACEABILITY_TABLE_TOO_SMALL",
    "TEST_MATRIX_TOO_SMALL"
  ],
  "head": "UNKNOWN"
}
```

The supplied ZIP digest is retained as input evidence and was not recomputed
from repository bytes in this local design-artifact environment.
