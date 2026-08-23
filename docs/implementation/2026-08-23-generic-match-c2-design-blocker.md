# Generic Match C2 final semantic owner design blocker

Date: 2026-08-23

Inspected Git commit: `c23dddb439b8ecf45b6161791ceb1e4e281ca9b6`

Initial repository state: clean `main`, equal to `origin/main`

Audit authoring state: documentation-only changes for this blocker record and
its linked request; no production, test, fixture, manifest, branch, or
worktree change.

## Status

Generic Match C2 is `BLOCKED` on result-changing owner construction and phase
ordering. C1 remains accepted and is not reopened by this finding.

The independently throwable design request that can close the blocker is:

[Lang-01.5.1.1.2.1.1.1.1.1.1.1.2.1 — final semantic owner construction and seal correction](../reviews/requests/2026-08-23-lang-01.5.1.1.2.1.1.1.1.1.1.1.2.1-final-semantic-owner-construction-and-seal-correction.md)

## Performed

- Read the accepted C2 schemas, cut/deletion matrix, and machine source
  inventory.
- Inspected current typed definitions, constructors, readers, validation,
  transcript consumers, compiler/runtime-plan consumers, and representative
  tests for project items, Entry references, variants, record patterns, typed
  bindings, every Select family, StageLook, Effect, View calls, Style values,
  dialogue rich text, and PostfixBracket.
- Compared the final-analysis phase with checked Entry catalog construction and
  compiler publication order.
- Inspected the existing checked callable join, project runtime nominal
  projection, environment nominal records, Character definition index,
  rich-text report, View element owner, and `ViewSpecifiedValue` owner.
- Verified the initial branch/revision/clean-state tuple and performed a final
  documentation diff check after authoring this note and the request.

## Passed

- Initial `git status --short`: empty.
- `git rev-parse HEAD`:
  `c23dddb439b8ecf45b6161791ceb1e4e281ca9b6`.
- `git rev-parse origin/main`:
  `c23dddb439b8ecf45b6161791ceb1e4e281ca9b6`.
- `git branch --show-current`: `main`.
- `git diff --check`: passed; the working tree contains no tracked diff.
- Both new Markdown files passed `git diff --no-index --check`, have no
  trailing whitespace, and end with a newline.
- Typed inventory established all current producers and compile fanout needed
  to formulate the linked request without editing production.

## Failed

No executed validation command failed. This was a read-only production audit,
not an implementation attempt.

## Blocked

The exact blockers are:

1. final analysis cannot retain `CheckedEntryBindingDigest` because the checked
   Entry catalog is currently constructed after final analysis and
   verification;
2. the canonical runtime nominal projection is exposed only on sealed
   `FinalSemanticAnalysis`, while C2 field/case construction needs that evidence
   before sealing; directly calling it from construction would create a phase
   cycle;
3. environment record fields have no declaration-ordered typed owner;
4. View modifier success has no accepted catalog and compiler lowering rejects
   it;
5. removing the Postfix selected `ExprId` would break compiler runtime-plan
   lowering in `arcweft-compiler/src/lower.rs` and runtime reachability in
   `arcweft-compiler/src/lower/reachability.rs` unless private lookup evidence
   is retained;
6. parent text says 27 current Style variants while production has 26;
7. the parent selected-case sketch duplicates a case row instead of using one
   owner table plus ordinal accessor, and its proposed family/receiver names
   duplicate existing types; and
8. complete rich-text and Postfix semantic digests depend on C3 checked child
   digest closure and cannot honestly be credited to C2.

These are not permission to add a late patch, parallel catalog, name hash,
sorted-map surrogate, caller digest, or optional compatibility path.

## Not run

- Cargo format, check, test, Clippy, workspace, Tier 2, generated-artifact, and
  structure gates were not run because no production Rust or test change was
  authorized.
- No returned package validator or negative corpus was run; the follow-up
  request has not yet been answered.

## Required decision order

1. private two-phase final-analysis/Entry seal, atomic publication, and exact
   error precedence;
2. one private runtime nominal projection context;
3. ordered typed environment record authority or explicit scope removal;
4. fail-closed View modifier deletion, preferably, or a real catalog;
5. exact reuse of `DeclarationIdentityFamily`, `CallableReceiverMode`, the
   checked callable join digest owner, one shared field identity, and selected
   variant owner-plus-ordinal access;
6. C2 fact construction versus C3 rich-text/Postfix digest sequencing; and
7. deletion-driven subcuts and executable acceptance tests.

The linked request also fixes these constraints: `ViewSpecifiedValue` uses an
owner-defined exhaustive encoder and pairwise uniqueness tests without a
literal count gate; Postfix keeps its selected `ExprId` lookup-only until C3
adds the typed child digest; `RecordElement` is deleted while `0x0406` remains
reserved; same-cut opaque IDs require exact typed inputs, trailing-NUL BLAKE3
domains, private construction, and generation validation; rich-text pairing
uses stable accepted token ordinals rather than raw tag IDs; and accepted C1 is
not redesigned absent a concrete flaw.

## Non-goals

- no C2 implementation or partial completion credit;
- no accepted C1 redesign;
- no C3 transcript, C4 coverage, C5 publication, runtime task-plan, wire,
  persistence, or compatibility work; and
- no source spelling or raw HIR ID in semantic identity.
