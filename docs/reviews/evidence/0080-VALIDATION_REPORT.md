# Validation report

Date: 2026-08-03

Status: `READY_FOR_IMPLEMENTATION`

This report distinguishes package/design validation actually performed from
future production validation required during implementation.

## Repository and instruction validation performed

- Queried GitHub `main` immediately before archive construction. The latest
  observed commit remained
  `aa983fda6b0de36d2f6867085ecdc95e630c5d99`
  (`Scope agent instructions and standardize Git workflow`).
- Compared that commit directly with the first-pass baseline
  `70e24164373e7898ff9ef83f56f4c48523ce108e`. Git reported one intervening
  commit and no changed Rust, Flow grammar, HIR, sema, request, intake, or
  predecessor-package file. The changed files were scoped instructions,
  Git/test/structural-audit policy, and unrelated maintained-document wording.
- Read the applicable current root, `crates/`, `docs/`, `docs/reviews/`, and
  `docs/implementation/` `AGENTS.md` files, plus `docs/README.md`,
  `docs/reviews/README.md`, the crate map, current test policy, and current
  structural-audit policy.
- Read the supplied Rust Skill completely. Local SHA-256:
  `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`.
- Read the supplied Arcweft project premise completely. Local SHA-256:
  `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`.
- Inspected the immutable Git commit tree through the GitHub connector. No
  mutable repository checkout was created, so the recorded state is
  `CLEAN_IMMUTABLE_COMMIT_TREE`; local worktree dirty state is not applicable.

## Request-copy validation performed

The four standalone repository copies were reconstructed byte-for-byte and
validated using Git blob hashing:

| Member | Git blob SHA-1 |
| --- | --- |
| `PARENT_REQUEST_COPY.md` | `7e51718bd018c3f363a08d3d9c5d07f0e3286279` |
| `PRIMARY_REQUEST_COPY.md` | `54fa4f14f842085be1e8763a2b0080b255f64a02` |
| `CORRECTION_REQUEST_COPY.md` | `15da00ca6de4b9bbc399fc9312c745ab722f531f` |
| `REJECTED_RETURN_INTAKE_COPY.md` | `9571d010ab9cdc3f4ec2ce498462fcd8ed5d2c9f` |

Each value equals the corresponding file blob at the inspected commit.

## Schema and matrix validation performed

A local package validator parsed every TSV as strict UTF-8 tabular data and
checked exact column cardinality, non-null fields, and unique row IDs.

| Member | Data rows |
| --- | ---: |
| `ACCOUNTING_LIMIT_TRANSACTION_MATRIX.tsv` | 37 |
| `CONSUMER_DELETION_INVENTORY.tsv` | 60 |
| `CONTRACT_CLAUSE_MATRIX.tsv` | 9 |
| `FLOW_THREAD_ITEM_MATRIX.tsv` | 16 |
| `IDENTITY_SIGNATURE_MATRIX.tsv` | 34 |
| `LOCAL_INSTRUCTION_EVIDENCE.tsv` | 2 |
| `PREDECESSOR_LEDGER.tsv` | 15 |
| `REPOSITORY_EVIDENCE_LEDGER.tsv` | 43 |
| `REQUIREMENTS_TRACEABILITY.tsv` | 38 |
| `SCOPE_LOCAL_MATRIX.tsv` | 12 |
| `SOURCE_RECOVERY_DIAGNOSTIC_MATRIX.tsv` | 40 |
| `TEST_MATRIX.tsv` | 306 |

Performed semantic cross-checks:

- the contract matrix contains exactly, in normative order, `requires`,
  `ensures`, `invariant`, `assume`, `reads`, `effects`, `no_effect`,
  `modifies`, and `decreases`;
- the body matrix contains exactly the sixteen required
  `HirThreadFlowItem` variants;
- each of those sixteen variants has an explicit ordinary-Flow test and an
  explicit `ThreadExpression` test;
- every one of the 148 identity/signature, contract, body-item, scope/local,
  source/recovery/diagnostic, and accounting/transaction rows is referenced by
  at least one exact test row after deterministic range expansion;
- every traceability row is exactly `CLOSED`;
- the consumer inventory contains syntax, attached syntax, HIR, source index,
  sema, project, compiler, verifier, runtime-plan, formatter, LSP, CLI,
  Agent/debug, cache, and test migration owners;
- positive, malformed, negative, recovery, compile-fail, stale, foreign,
  cancellation, panic, rollback, exact-limit, first-one-over, multi-module,
  incremental reorder, source-query, and consumer-integration evidence is
  present;
- `HirFlowItem` explicitly stores `result_local: Option<HirFlowResultLocal>`;
- nested branch bodies have the non-colliding final owner
  `HirThreadBodyOwner::NestedScope(ScopeId)`;
- attached Flow and Thread body records expose no tail or value accessor; and
- no production-code, patch, Cargo, fixture, schema-wire, compatibility, or
  source-gate artifact is present.

All package text was checked as UTF-8. Generated members use LF line endings.
Every member except the deliberately exact four-byte `OPEN_QUESTIONS.md` ends
with a newline. `OPEN_QUESTIONS.md` is exactly hexadecimal `6e 6f 6e 65`
(`none`), and `FINAL_STATUS.md` resolves exactly to
`READY_FOR_IMPLEMENTATION`.

## Predecessor verification scope

Accepted predecessor identities, byte lengths, member counts, and internal
manifest outcomes were checked against repository intake notes that record
direct archive verification. The base Proof ZIP was also fetched as binary
GitHub evidence. The connector did not expose a local file reference suitable
for a second independent unzip, so this package does not claim such a second
unzip. `PREDECESSOR_LEDGER.tsv` and `EVIDENCE_SCOPE.md` state that boundary
explicitly.

## Final archive validation performed

After deterministic construction, the archive was reopened and checked for:

- the exact filename-sorted member set declared by this package;
- no directory members, absolute paths, traversal paths, duplicate names, or
  case-folded name collisions;
- successful decompression and CRC for every member;
- exact UTF-8 member bytes matching the construction directory;
- a filename-sorted `MANIFEST.sha256` containing every non-self member exactly
  once, with a 64-hex SHA-256 and a zero-padded 12-digit exact byte length;
- exact member hash and byte-length agreement for every manifest row;
- the four request/intake Git blob identities above;
- exact status and open-question bytes; and
- absence of adjacent required sidecars.

The final external SHA-256 and byte length are reported with the returned
artifact rather than embedded recursively inside the archive.

## Production validation not run

No production Rust, tests, fixtures, manifests, stable chapters, branch, patch,
PR, or implementation overlay was created. Therefore the following commands
were intentionally **not run** and are not claimed as completed evidence:

- Rust compilation, focused crate tests, workspace check, or Clippy;
- `just test-workspace`;
- Tier 2 runtime/Agent tests; and
- the structural audit.

`TEST_MATRIX.tsv` and `IMPLEMENTATION_ORDER.md` define those required future
implementation gates under the current repository policies. This design-only
status is not a blocker to the package's decision completeness.
