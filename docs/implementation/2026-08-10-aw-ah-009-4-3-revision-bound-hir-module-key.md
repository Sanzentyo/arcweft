# AW-AH-009.4.3 revision-bound HIR module key

Date: 2026-08-10

Inspected Git commit: `bef4a98901ee0063b8aae034869607e67608b304`

Working-tree state at inspection: dirty on `main` with this coherent HIR
source-identity migration; the sole repository checkout and shared Cargo
target are in use, and no subagent is involved.

## Outcome

The first production frontier of the returned AW-AH-009.4.3 source-site line
identity contract is implemented. Public `HirModuleKey` now owns the exact
`SourceDocumentIdentity`, including revision and source length, rather than
only the logical `SourceDocumentId`.

`LoweringRequest::try_new` compares the exact key identity with the attached
`ParsedSource` before HIR staging or allocation. A different logical document
reports `SourceDocumentMismatch`; the same document at a different revision
reports `SourceIdentityMismatch` with both complete typed identities. The
retained syntax root is then revalidated against that same admitted identity.

The HIR database no longer overloads this exact public key as its lineage map
key. Its private `HirModuleRegistryKey` contains package, canonical module
path, and logical document ID. It is used only to find the current module
lineage and reconcile revisions. Exact snapshots, lowering requests, modules,
project leases, and public current lookups retain `HirModuleKey`.

Consequently:

- `current(exact_key)` cannot return a module from another source revision;
- failed revised proposals leave the prior exact key current;
- successful revised publication is visible only through the revised exact
  key;
- stale project leases compare against the current logical lineage and report
  `StaleModuleLease`, without weakening exact public lookup; and
- every repository consumer constructs a module key from the admitted source
  identity rather than projecting only the document ID.

## Contract classification

| AW-AH-009.4.3 boundary | State | Current owner |
|---|---|---|
| exact source revision in public HIR module key | `LANDED_VALIDATED` | `HirModuleKey` |
| pre-staging source revision rejection | `LANDED_VALIDATED` | `LoweringRequest::try_new` |
| logical module lineage reconciliation | `LANDED_VALIDATED` | private `HirModuleRegistryKey` |
| exact project lease/current validation | `LANDED_VALIDATED` | `HirProjectModule` / `HirProject` |
| module-local dialogue line candidates | `NOT_STARTED_IN_THIS_CUT` | selected contract requires `HirDialogueLineCandidates` |
| accepted project dialogue line inventory and collision transaction | `NOT_STARTED_IN_THIS_CUT` | selected contract requires `AcceptedDialogueLineInventory` |
| CharacterDialogue runtime/View/Agent/save consumers | `DEPENDENCY_BLOCKED_IN_SEQUENCE` | require the accepted line inventory first |

The AW-AH-009.4.2 typed dialogue content application owner is already present
in final HIR as `HirDialogueContentApplication` and typed postfix-bracket
evidence. This cut does not add another content model or restore source
reconstruction.

## Passed validation

- `cargo check -p arcweft-lang-hir --all-targets --all-features`: passed;
- the seven initially exposed revision-sensitive rollback/retry, project lease,
  capture, slot-limit, Flow, expression, and Pattern tests: all passed after
  exact-key consumer migration;
- `source_revision_mismatch_fails_before_database_staging`: passed and proves
  typed same-document/different-revision rejection with unchanged database
  state; and
- `cargo test -p arcweft-lang-hir --lib --all-features`: 842 passed, 0 failed,
  8 ignored;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed; and
- `just structure-audit` and `just structure-audit-gate`: passed at 2,113
  files, 1,990 Rust files, 979,700 Rust LOC, 94 workspace packages, 181 review
  triggers, and zero blocking findings.

The touched size-trigger files were inspected as mechanical exact-key caller
migrations. This cut adds no owner to those files and does not increase their
semantic responsibility.

The full AW-AH-009.4.3 Tier 2 gate is intentionally not claimed by this
frontier. This cut changes HIR source admission and project lease identity but
does not yet connect the package's runtime, authored View, Agent/MCP, codec, or
save/replay paths. `just test-tier2` remains required at the accepted line
inventory integration cut rather than being reported as evidence for an
unimplemented consumer graph.

## Explicit non-goals

- no `AcceptedDialogueLineInventory` or project collision policy in this cut;
- no runtime, View, Agent, codec, or save/replay projection before that
  dependency is accepted;
- no source reread, source-spelling identity, parallel revision side table, or
  public logical-key fallback;
- no compatibility alias, dual reader, shim, CSS/Takumi path, removed-syntax
  diagnostic, or source gate; and
- no implementation of an unreturned correction contract.
