# AW-AH-009.2.1.1 launch profile overlay reconciliation

## Status

Implementation is in progress. This note records reviewable cuts without
treating the package as complete before its full acceptance matrix is covered.

## Completed cut: deterministic profile selection

`arcweft-launch` now owns the typed `LaunchProfileSelection` policy and selects
profiles with the package-defined precedence:

1. explicit selection is exact and never falls back;
2. automatic selection uses a valid manifest default;
3. otherwise it retains an existing previous profile;
4. otherwise it chooses the lexicographically first declared profile;
5. an invalid declared default and an empty profile map are distinct errors.

Direct tests cover each branch. Focused tests, all-target/all-feature checking,
clippy with warnings denied, and formatting have passed for this cut.

## Remaining package work

The following remain part of AW-AH-009.2.1.1 and are not completion claims for
this cut:

- source-backed adapter registry decoding and checked adapter inventory;
- immutable document snapshots and exact import-closure loading;
- overlay-first topology resolution without directory enumeration in the LSP
  request path;
- workspace-keyed profile slots, input tokens, permits, and accepted-candidate
  identity rules;
- begin, commit, fail, and capture transaction APIs;
- failed-rebuild eligibility and accepted-pointer preservation;
- package diagnostics, limits, tamper cases, concurrency cases, and the complete
  acceptance matrix.

The subsequent AW-AH-009.2.1.2 diagnostics reconciliation and AW-AH-009.2.1.3
shared request-budget reconciliation remain ordered after this package.
