# Validation report

## Artifact basis

- Contract request: `Lang-01.5.1.1.2.1.1.1.1.1.1.1.3`
- Repository snapshot inspected: `515bb071437c3af053f1560c3119906dc8002efc`
- Production patch in archive: none
- Validator dependencies: Python standard library only
- Validator behavior: read-only for the package and optional repository;
  negative tests mutate temporary copies only

## Performed package validation

| Check | Result |
|---|---|
| full required member inventory | PASS |
| manifest sizes and SHA-256 values | PASS |
| `MANIFEST.sha256` and `CHECKSUMS.sha256` | PASS |
| exact `FINAL_STATUS` and `OPEN_QUESTIONS` | PASS |
| exact version-one domains/tags/limits | PASS |
| exact final Rust-shaped row/protocol fields | PASS |
| absence of public raw digest constructor | PASS |
| absence of task-plan self/expected field | PASS |
| absence of raw core View projection | PASS |
| absence of caller/general byte sink API | PASS |
| dependency machine proof: core does not depend on View/bundle | PASS |
| exact fifteen executable table rows | PASS |
| extracted-directory validator run | PASS |
| all included negative mutation cases | PASS |
| returned ZIP filename/wrapper/path safety/readback | PASS |
| returned ZIP validator run | PASS |

## Negative self-test corpus

The included `tools/negative_self_tests.py` ran fourteen temporary mutation
cases and required every one to fail validation:

1. non-ready final status;
2. non-`none` open questions;
3. payload tamper without manifest update;
4. missing authoritative task-plan domain;
5. self digest added to `RuntimeTaskPlan`;
6. public raw digest constructor;
7. raw core View identity projection;
8. caller/general byte sink;
9. public expected-key row;
10. machine dependency changed to core -> View;
11. version marker changed from one;
12. required cycle proof removed;
13. ZIP parent-traversal path; and
14. ZIP case-fold collision.

All were rejected for the intended blocker.

## Repository-aware mode

`tools/validate_contract.py --repo <checkout>` is included but was not run in
this environment because repository inspection used the private GitHub
connector rather than a local checkout. The mode is read-only and requires:

- checkout HEAD exactly
  `515bb071437c3af053f1560c3119906dc8002efc`;
- current AGENT/source evidence paths;
- structured `cargo metadata --format-version 1 --no-deps` proof that core has
  no View/bundle dependency and bundle depends on core and View.

No production Cargo build, tests, fmt, Clippy, generated fixture comparison, or
platform runtime validation is claimed by this design-only archive. Those are
implementation acceptance requirements in `TEST_MATRIX.md`.
