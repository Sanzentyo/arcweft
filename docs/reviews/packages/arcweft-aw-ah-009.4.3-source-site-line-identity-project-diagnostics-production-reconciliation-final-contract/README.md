# AW-AH-009.4.3 final production-reconciliation contract

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
PRODUCTION_CHANGES_INCLUDED=0
BASELINE_GIT=27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9
JJ_CHANGE_ID=UNAVAILABLE_FROM_REMOTE_GIT_SNAPSHOT
```

This archive is the decision-complete implementation contract for source-site
DialogueLine identity, package-aware HIR ownership, project-wide transactional
collision acceptance, text-key derivation, revision-bound diagnostics,
invalidation, rename behavior, and direct migration.

It consumes the completed AW-AH-009.4.2 contract whose locally verified archive
SHA-256 is:

```text
05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8
```

The package changes no production source. It fixes exact types, ownership,
algorithms, errors, limits, tests, and implementation order so implementation
can proceed without selecting alternatives.

## Normative documents

- `FINAL_CONTRACT.md` — precedence and complete decision ledger.
- `PRODUCTION_RECONCILIATION.md` — current-production defects and selected
  replacement.
- `SOURCE_OWNER_MODEL.md` — package/module/document/flow/callable/scope source
  ownership.
- `LINE_ID_BUILDER.md` — checked candidate construction and relative resolution.
- `PROJECT_COLLISION_TRANSACTION.md` — the sole project acceptance transaction.
- `DIAGNOSTIC_MODEL.md` — structured diagnostic identity and SourceSpan
  projection.
- `TEXT_KEY_AND_RENAME.md` — text keys, references, and rename behavior.
- `LIMITS_INVALIDATION_AND_CACHE.md` — fixed budgets and generation coherence.
- `MIGRATION_AND_DELETION.md` — exhaustive direct replacement inventory.
- `IMPLEMENTATION_HANDOFF.md` — compiling frontiers, commands, and completion
  gate.
- `TEST_MATRIX.md` — 100 exact named tests.
- `REQUIREMENTS_TRACEABILITY.md` — request-to-contract mapping.
- `REPOSITORY_EVIDENCE.md` — inspected revision, files, hashes, and verification
  limits.
- `OPEN_QUESTIONS.md` — exactly `none`.
- `FINAL_STATUS.md` — final machine and human status.
- `MANIFEST.txt` — sorted member hashes with a zero self-entry.

## Non-negotiable exclusions

No compatibility alias, deprecated helper, dual reader, parallel line
inventory, source gate, source-spelling scan, `.say` recognizer, stringly
character recovery, CSS route, or Takumi route is authorized.
