# Verification scope

## Directly verified during this return

- full attached request content and its resolved disposition;
- full provided Rust skill;
- current root `AGENTS.md` and Git-only/typed-authority policy;
- current baseline commit identity from the repository snapshot;
- exact source definitions for dialogue profile, policy, revision, launch
  manifest, accepted manifest/source-map projection, compiler admission, and
  focused compiler tests;
- mandatory document presence in the ZIP;
- deterministic UTF-8/LF payload generation;
- `OPEN_QUESTIONS.md` exact byte content;
- all `MANIFEST.txt` SHA-256 and byte-size entries;
- ZIP CRC integrity and clean extraction;
- external archive SHA-256 sidecar.

## Verified through maintained repository records, not rerun

- complete workspace check;
- Clippy with warnings denied;
- workspace tests;
- Tier 2;
- structural audit;
- broad runtime/tooling/renderer migration closure.

These are labelled “recorded pass,” never “run by this return.”

## Not verified in this return

- compiling the current workspace from a local checkout;
- platform-specific native/Web targets;
- live runtime hot replacement;
- actual CLI/LSP binary execution;
- current post-baseline changes, if main advances after the recorded SHA.

## Consequence

The archive is decision-complete and grounded in current source, but anyone
using it after the baseline must rerun `VERIFICATION_PLAN.md` against the actual
checkout. No unrun tier is silently promoted to passed.
