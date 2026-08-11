# Validation report

## Baseline

- repository: `Sanzentyo/arcweft`
- main: `004ff3d69f241954eb808985878c348b165a815c`
- correction request blob:
  `a57f0a4bd2419ef49822a2adf6886798d5e2066b`
- rejected intake blob:
  `e4b35455d95d0a12677127e8a797a4771d19a291`
- primary request blob:
  `162a83984b27b8458e3380a15c17642111b080cc`
- rejected archive identity:
  `BC8DE35E8C4D69008344EC44B9CFF1C5C59EE17ECB2CA54006B0ECF6EE923B50`

## Predecessor evidence scope

Repository-retained predecessor archive identities, byte lengths/member counts,
intake manifest results, Git blob identities, normative archive member names,
and result-changing contract rows were directly reconciled against current
source and intakes. The audit table records the exact archive authority and the
specific member/contract result used by this replacement.

This package does not reuse the rejected archive.

## Mechanical checks performed at build time

- exact archive name;
- unique safe members;
- deterministic ZIP member order/timestamps;
- every non-manifest member byte length and SHA-256 recorded and recomputed;
- `FINAL_STATUS.md == "READY_FOR_IMPLEMENTATION\n"`;
- `OPEN_QUESTIONS.md == "none"` (four bytes);
- unique test IDs and matrix IDs;
- all traceability rows `CLOSED`;
- all contradiction rows `PASS`;
- normative schema/matrix files contain no duplicate argument-index or parallel
  limit type;
- normative source contract contains no Call-specific second source map/query;
- normative fixtures use `name = value` and postfix `value...`;
- required cursor rows R04/R05/R08/R09/R13/R14 are present;
- exact ordinary/RichText/type/candidate/depth/recovery/Proof witness boundaries
  are present and reachable;
- deletion matrix covers parser, attachment, HIR, source, sema, resolver, facts,
  signature, LSP, Proof, tests, and audit consumers.

## Design-only boundary

No Rust/Cargo/fixture/schema/stable repository file was edited. No branch, PR,
patch, overlay, source gate, adjacent sidecar, or implementation artifact was
created.
