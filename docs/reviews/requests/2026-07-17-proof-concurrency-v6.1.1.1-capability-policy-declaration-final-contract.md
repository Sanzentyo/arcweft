# Request: Capability policy declaration final contract

## Sequence position

This is proof-concurrency v6.1.1.1. It is split from Stage 1 of
proof-concurrency v6.1.1 after the concrete `extern capability` type,
function, and effect grammar is implemented. It must be resolved before Stage
1 can claim that the currently documented `CapabilityItem` vocabulary is
complete, but it does not block the already-defined capability members or
other independently designed Stage 1 families.

## Why this split is needed

`docs/01-language/entries-and-capabilities.md` names
`CapabilityPolicyDecl` as a possible member of an `extern capability` body,
but gives it no production, example, typed payload, ownership rule, semantic
effect, diagnostic contract, or runtime representation. The current public
AST stores only capability functions, and the proof-concurrency v6.1.1 final
`SyntaxKind` inventory contains no dedicated capability-policy node.

Inventing a spelling or storing another raw string would conflict with the
repository's typed-boundary and top-level-reduction direction. The design may
also conclude that policy belongs in a typed build/profile manifest or is
fully derivable from capability functions and their effect rows. That choice
must be explicit before production implementation.

## Required decisions

1. Decide whether an author-facing capability policy declaration is needed at
   all. Include concrete uses that cannot be expressed by capability function
   signatures, effect rows, host adapter contracts, or typed build/profile
   policy.
2. If it remains source-authored, define its exact grammar, name/identity
   model, typed fields, repetition and ordering rules, visibility, defaults,
   and interaction with documentation and attributes.
3. Decide whether it is nested only inside `extern capability`, belongs in a
   typed manifest/profile, or is derived. Do not create dual source/manifest
   ownership.
4. Specify whether existing lossless syntax kinds can represent it without an
   ambiguous projection. If a new kind is necessary, define its
   identity-bearing classification and exact parent/child `SyntaxRole`s.
5. Define the public AST and HIR representation, or explicitly state that the
   policy has no AST/HIR node because it is manifest-owned or derived.
6. Define sema rules: duplicate and conflicting policies, effect-row
   consistency, capability availability, visibility, profile overlays, and
   host-adapter compatibility.
7. Define whether any runtime-plan or serialized runtime data is required.
   Prefer compile-time policy and existing typed manifest facts when runtime
   state is unnecessary.
8. Define stable structured diagnostics and recovery boundaries for malformed,
   unknown, misplaced, duplicate, and contradictory policy entries.
9. State the deletion/migration rule for the existing documentation placeholder.
   Arcweft has no released compatibility requirement for an inferred spelling.

## Implementation order to specify

1. Evidence-based keep/delete/derive/manifest decision.
2. Final source or manifest schema and ownership boundary.
3. Lossless grammar and typed AST attachment, if source-authored.
4. HIR lowering and semantic validation, if the policy participates in HIR.
5. Project/profile/adapter and runtime-plan integration only where required.
6. CLI, LSP, Agent, formatter, and audit presentation.
7. Delete the placeholder grammar reference or replace it directly with the
   final contract and canonical examples.

## Tests to specify

- Canonical positive cases for every retained policy form and combination.
- Missing names, fields, values, delimiters, and malformed nested values retain
  exact typed recovery without hiding following capability members or
  top-level declarations.
- Duplicate, contradictory, unavailable-effect, wrong-profile, and wrong-host
  cases return deterministic structured diagnostics.
- Source, AST, HIR, sema, project/profile, adapter, and runtime views agree on
  one owner and one identity when those layers participate.
- A manifest-owned design has typed codec round trips and tamper/unknown-field
  rejection; it must not be mirrored by a source declaration.
- A derived design proves the same result from capability functions/effect rows
  and has no independently mutable policy record.
- Formatter and LSP behavior use typed grammar/semantic APIs.
- Removal or rejection is tested through parser/compiler behavior and public
  API compile failures, never by scanning checked-in source text.

## Constraints

- Do not redesign the already implemented lossless `extern capability`
  header, opaque type member, function signature, curried parameter, return
  type, or braced effect-expression substrate unless current behavior exposes
  a concrete flaw.
- Do not add raw policy strings, `signature_tail` reparsing, name-based special
  cases, compatibility aliases, dual readers, deprecated nodes, or a permanent
  removed-spelling diagnostic.
- Keep `arcweft-lang-syntax` syntax-only, HIR/sema/runtime in their owning
  crates, and manifest codecs Sans I/O.
- Preserve abstract capability identity as the host security/effect boundary.
- Do not redesign Lang-01.2 callable/state/entry work, Lang-01.3 live-source
  work, Lang-01.5 build/profile extraction, View ownership, or proof-runtime
  identity unless a concrete dependency is demonstrated.
- Do not use source gates as acceptance evidence.

## Expected output

- A keep/delete/derive/manifest decision with motivating use cases.
- Exact final grammar or typed manifest schema, including identity and
  ownership.
- AST/HIR/sema/project/runtime participation table.
- Structured diagnostics and recovery contract.
- Direct implementation/deletion order and complete positive, negative,
  recovery, round-trip, and tamper test matrix.
- Canonical documentation examples with no provisional alternate spelling.
