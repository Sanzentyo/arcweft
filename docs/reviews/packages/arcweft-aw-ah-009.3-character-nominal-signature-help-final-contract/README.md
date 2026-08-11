# AW-AH-009.3 character nominal signature-help final contract

## Delivery decision

- **Status:** `READY_FOR_IMPLEMENTATION`
- **Outcome:** `IMPLEMENTATION`
- **Repository Git commit inspected:** `76d39983ad8770a87d6e81745785b6b362a381b4`
- **Repository Jujutsu change:** `unavailable`
- **Production changes in this delivery:** none
- **Open result-changing decisions:** zero

This archive is the decision-complete design and implementation handoff for one
position-aware native signature-help query. The query is owned by
`arcweft-lang-sema`, consumes the atomically accepted `RegisteredSemanticWorld`
and its `ProjectSymbolTable`, resolves all currently applicable call surfaces
through one shared semantic resolver, and exposes structural
`CharacterNominalType` values without parsing display labels.

The selected identity path is **`SourceDocumentIdentity` plus parser-retained,
exact typed call and argument-list ranges**. `SourceSnapshotId` and stable
`SyntaxNodeId` attachment are not required. Proof 01.1.1 therefore remains
independent and is not an implementation prerequisite for this contract.

## Central repository finding

Current main has real character-aware authoring surfaces, so an explicit
non-goal is not supportable. In particular, the presentation `show` special
form and dialogue speaker/content calls carry a character owner and a `look`
argument. Current main does **not** yet pass a structural character nominal
expected type into those `look` expressions: both current checker paths call
ordinary expression checking. The implementation must correct that gap by
moving call-shape and parameter-expectation behavior into sema-owned typed call
specifications used by both the checker and the signature query. It must not
add a signature-only workaround.

## Frozen decisions

1. One public `arcweft_lang_sema::signature::query_signature` API owns semantic
   signature lookup.
2. One internal `arcweft_lang_sema::call_resolution` path owns target
   precedence for type checking and signature help.
3. Parser-owned `CallExpressionSyntax` and `ArgumentListSyntax` retain exact
   byte ranges, separators, and recovery boundaries; no source search is used.
4. Project source symbols take precedence over accepted environment/adapter
   bindings after special-form precedence is applied. Adapter metadata is
   normalized into the accepted environment and is never queried by a second
   LSP resolver.
5. `CharacterNominalType` is the only look/part/variant type identity. Authored
   aliases are display context only; canonical type labels use
   `source_label()` and are never parsed back.
6. Successful results and stable non-applicability may be cached only under an
   exact accepted-generation, world, revision, character inventory, document,
   LSP version, and byte-offset key.
7. Every limit, recovery rule, stale condition, diagnostic ordering rule, and
   LSP error mapping is fixed by this archive.
8. The word-at-cursor Rust-adapter fallback is deleted after migration.

## Archive contents

- `FINAL_CONTRACT.md` — normative API, resolver, position, recovery,
  presentation, and error contract.
- `SURFACE_INVENTORY.md` — exhaustive current call-family classification.
- `IDENTITY_CACHE_AND_LIMITS.md` — source/world identity, cache, invalidation,
  limits, work accounting, and failure publication.
- `IMPLEMENTATION_HANDOFF.md` — compiling cuts, crate ownership, deletion
  order, validation, and audit commands.
- `TEST_MATRIX.md` — direct positive, negative, recovery, stale, precedence,
  deterministic, exact-limit, and one-over tests.
- `REQUIREMENTS_TRACEABILITY.md` — request-to-contract coverage.
- `REPOSITORY_EVIDENCE.md` — current-main observations and verification scope.
- `OPEN_QUESTIONS.md` — fixed to `none`.
- `FINAL_STATUS.md` — machine-readable outcome plus verification boundary.
- `MANIFEST.txt` — sorted SHA-256 inventory; its own digest is zero-filled by
  contract.

## Verification boundary

The private repository was inspected at the commit above through the GitHub
connector. The Rust skill and current root `AGENTS.md` were read in full, and
the request-named implementation notes and source owners were inspected. This
archive contains no Rust, Cargo, schema, fixture, patch, overlay, checkout, VCS
metadata, or build output. Consequently no production compilation claim is
made. Archive membership, line endings, required file names, open-question
content, manifest digests, ZIP integrity, and the external ZIP digest are
mechanically verified during packaging.
