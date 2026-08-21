# Verification scope

## Performed in this environment

- Read the mandatory correction in full (373 lines).
- Read the Rust skill in full (56 lines).
- Read the project premise.
- Queried the private `Sanzentyo/arcweft` repository through the GitHub
  connector and resolved latest main to `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009`.
- Read root/docs/reviews/implementation/crates `AGENTS.md` constraints.
- Read the parent request, frozen mirror, scheduler/Need/AWBC maintained
  contracts, and the return-intake audit.
- Inspected current production source at `3670625a02b9e7e8578b57fc7b148a1758a17dba` for:
  `RuntimeValue`, `AwbcRuntimeValueSnapshot`, `RuntimeFunctionBody`,
  `RuntimeIterator`, `RuntimeSeq`, `DenseSeq` constructors, opaque/reduction/
  Agent values, `RuntimeCheckedType`, `RuntimeAgentOperationalType`,
  `TaskSpec`, event ordering, scheduler dependencies, current host adapter
  timing, 38 `HirExprKind` families, 13 pattern families, checked expression
  resolutions, and checked callable identity.
- Generated every machine table and Markdown file in this package.
- Executed `tools/validate_package.py` against the directory.
- Executed all twelve negative in-memory self-tests.
- Re-ran validation after manifest generation.
- Built the final ZIP deterministically and verified every ZIP member against
  `MANIFEST.json`.

## Not performed

- No production checkout was available in the execution container, so no
  production `cargo fmt`, `cargo clippy`, `cargo test`, or compile was run.
- `rustc` and `cargo` were not installed in the container. The Rust schema file
  is a normative design excerpt, not a standalone compilation unit.
- The retained parent ZIP bytes could not be streamed into the container.
  Therefore this run did **not** locally rehash that archive. The repository
  intake records the parent as 197,348 bytes, 61 members, with SHA-256
  `034A2EEAB2D083B5BB4496F4EE63040B2F93B30ABDDA1B18E93138E28B65391B`; this package cites that repository evidence without
  upgrading it to a local-byte verification claim.
- No external network, worker, filesystem, audio, or adapter I/O behavior was
  exercised; the package specifies the Sans-I/O protocol and test rows for the
  later production implementation.

These limits do not weaken the package's structural validator claims: those
claims apply to the files actually present in this ZIP and are recorded in
`VALIDATION_OUTPUT.txt`.
