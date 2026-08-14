# Catalog-digest role-root return invalid as delivered

Date: 2026-08-13

Continues:
`docs/implementation/2026-08-13-generation-bound-authority-return-intake-and-residual-blocker.md`

Inspected Git baseline:
`cfcfb98ba185afd66052a2a98ef69001f0b01d82` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned archive intake

Retained archive:
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction-final-contract.zip`

SHA-256:
`0f4ea111edc521090bd34b8284e9480b6ac0c1d57d8e2043ed939ea6c6d55b94`

The 43,302-byte ZIP contains 25 files without a redundant wrapper. It has no
unsafe/rooted/drive/traversal path, symlink/reparse entry, or case-insensitive
collision. All 25 extracted files match their ZIP-member SHA-256 values and
every internal `MANIFEST.sha256` row passes. JSON and text metadata report a
design-only return, no production overlay, `OPEN_QUESTIONS=0`, and all
Arcweft-owned versions as `1`.

The package request copy has SHA-256
`3a2eeaec01994f984db20d6349dfea31eceb0c5cafc2b345099584117fb972c5`.
The maintained repository request has SHA-256
`0c570da664999507d1895813d65a707fb13726d48c489e8fd322c238a3361b78`.
The only textual difference is the maintained request's final clarification
that the earlier `RuntimeProducerRootError` cannot carry required
`RuntimeProjectRootId` failures and the return must select a project-capable
typed error owner. The returned package does not independently close that
missing error shape.

## Readiness adjudication

Sol max and direct full-package inspection classify this package as
`INVALID_AS_DELIVERED`, not merely implementation-incomplete. It does not make
the five result-changing decisions requested by the maintained request:

- CharacterCatalog and ViewRegistry payloads are called only
  "role-specific canonical typed catalog"; exact fields, typed keys, scalar
  encodings, order, anonymous/generated View behavior, tombstones, limits, and
  owner APIs are absent.
- No exact Stage/Portrait/Focus/Cleanup/Hook/RichText semantic and runtime type
  table, `CharacterDialogueRuntimeRoleDeclaration`, or standard registration
  path is defined.
- `RuntimeProjectRootFact`, `RuntimeProducerFact`, root-ID derivation, and the
  exhaustive plan/AWBC typed-table mapping are absent.
- No concrete project/producer construction-domain enum, project-shape issuer,
  AWBC `MakeRecord` authority coordinate, wire tag, lowering rule, or verifier
  mapping is defined.
- No shaped `RuntimeCheckedTypeError`, `ChoiceNoMatch`, `ChoiceAmbiguous`,
  branch mismatch evidence, shared budget behavior, or nominal-tree error
  mapping is defined.

Instead, the package introduces a new seven-variant
`RuntimeCatalogDigestRole`, `RuntimeCatalogDigestRoleRoot`, generic
`RawRoleCatalog`, `RuntimeGenerationAdmitter`, and
`plan_awbc_binding_digest`. These are absent from current production and
conflict with the accepted `.1.2.1` generation-contract body, project/producer
root declarations, specialized CharacterDialogue payload, and
`RuntimeGenerationIdentity` derivation. The package also changes the accepted
common scalar grammar from little-endian to big-endian without repository
evidence or a concrete defect.

`CONTRACT_METADATA.json` confirms that no repository checkout was available,
`repo_head` is null, and repository/AGENTS evidence rows are zero. Traceability
rows mark exact requirements closed with generic phrases but do not point to
the required concrete definitions.

## Disposition and next action

No production API, encoder, catalog, root, admission, AWBC, construction,
Choice, or execution change may be derived from this package. The previously
implemented lower role/ID and scalar substrate remains valid, but this return
authorizes no additional production gate.

A new child request would duplicate the still-unanswered request. Re-submit
the existing maintained request unchanged:

`docs/reviews/requests/2026-08-13-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction.md`

The replacement must use the same requested archive name, read the current
repository and applicable `AGENTS.md`, include the exact maintained request
copy/hash, and answer each required decision with repository-shaped concrete
types/tables/grammars rather than a generic authority graph.

## Validation performed

- source ZIP SHA-256 and byte length: verified;
- unsafe path, traversal, symlink/reparse, and case-collision preflight: passed;
- ZIP member versus extracted file SHA-256 parity: 25/25 passed;
- internal `MANIFEST.sha256`: passed;
- request-copy comparison: failed for the one maintained clarification stated
  above;
- all normative Markdown and metadata were inspected; and
- no Cargo, Clippy, workspace, structural, or Tier 2 command was run because
  no production implementation was authorized by this invalid return.
