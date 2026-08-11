# Evidence scope

## Repository access

Repository: `Sanzentyo/arcweft`

Inspected commit:

```text
aa983fda6b0de36d2f6867085ecdc95e630c5d99
```

Commit subject:

```text
Scope agent instructions and standardize Git workflow
```

GitHub exposed the immutable commit, repository files, blob identities,
accepted-package intake notes, and code-search results. The package records:

```text
Git commit: aa983fda6b0de36d2f6867085ecdc95e630c5d99
Git tree state: CLEAN_IMMUTABLE_COMMIT_TREE
Local mutable worktree: NOT_CREATED / NOT_APPLICABLE
```

This satisfies the Git-only evidence rule without inventing a Jujutsu identity
or a local dirty state.

Immediately before archive construction the GitHub `main` branch was queried
again. `VALIDATION_REPORT.md` records the observed head. If `main` advances
after archive construction, implementation must perform the normal Cut-0
current-owner comparison; this does not create a dual authority.


## Head-advance reconciliation

The first evidence pass inspected
`70e24164373e7898ff9ef83f56f4c48523ce108e`. Before package completion,
`main` advanced by one commit to the final inspected commit
`aa983fda6b0de36d2f6867085ecdc95e630c5d99`.

A direct Git compare reported that the intervening commit changed only scoped
agent instructions, Git/test/structural-audit documentation, and small
maintained-document workflow wording. It changed no Rust file, Flow grammar
file, HIR file, semantic consumer, request, intake, or predecessor package.
The new root and scoped `AGENTS.md` files, `docs/README.md`,
`docs/reviews/README.md`, crate map, test policy, and structural-audit policy
were read completely where applicable. The schemas and consumer findings
therefore remain result-identical; the evidence and workflow sections use the
new Git-only policy.

## What was directly inspected

- `AGENTS.md`, including the full source-gate, deletion, enum-owner, ZIP intake,
  structural audit, test, and completion rules;
- the correction, primary, parent, and rejected-return intake files;
- local Flow item/member decision, Flow-header implementation note, ordinary
  Flow gap and rejected-return intake;
- qualified identity, typed source owner, tail/generator, Call, Select,
  Dialogue, project, and public-switch notes;
- maintained grammar, contracts, block-scope, syntax, scenario, and dialogue
  chapters;
- current parser, attached grammar/role/attachment, clone-HIR, sema, project,
  verifier, compiler, runtime-plan, LSP, CLI, Agent/debug, tooling, cache, and
  test readers identified in the consumer inventory;
- accepted package identities and manifest verification outcomes from their
  repository intake notes.

## Binary predecessor verification scope

The predecessor ZIP bytes were not copied into this return. Their SHA-256,
member counts, and internal-manifest results are taken from repository intake
notes that report direct byte verification. The base Proof ZIP was also
retrieved through the GitHub connector as binary/base64 evidence, confirming
the repository blob exists at the inspected commit; the connector did not
provide a local filesystem reference suitable for a second independent unzip.

`PREDECESSOR_LEDGER.tsv` distinguishes:

- `REPOSITORY_INTAKE_VERIFIED`: archive and manifest were verified by the
  repository intake;
- `REPOSITORY_DECISION`: a returned package was rejected but the repository
  closed the affected implementation decisions;
- `REJECTED_NO_AUTHORITY`: retained only as historical evidence.

No claim is made that production Rust compiles against these future schemas.
This is design-only work; package validation checks the internal contract,
tables, request copies, manifest, and ZIP.
