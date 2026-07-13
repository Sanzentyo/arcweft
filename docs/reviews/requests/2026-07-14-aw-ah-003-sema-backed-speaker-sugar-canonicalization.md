# Request: AW-AH-003 sema-backed speaker-sugar canonicalization

Date: 2026-07-14

## Request status and independence

This is a new, standalone design request. It contains the accepted finding and
the decisions needed before implementation; the assignee must not need the
original audit ZIP or another request. AW-AH-003 is accepted as a real defect.
Do not spend this request re-auditing whether it exists.

The evidence snapshot was revision
`4204d25965129ced50abe82cf5de67d528b483d0`. Implementation targets the current
checkout, so exact line numbers may have moved.

## Finding and evidence

AW-AH-003 is a P1/high-confidence `confirmed_adhoc` finding:
`SpeakerPreset` is inferred by tooling from callee spellings and a handwritten
syntax-tree walk even though semantic analysis already owns a dedicated type.

Evidence locations in the audited snapshot:

- `crates/arcweft-tooling/src/speaker_presets.rs:465-479` recognizes constructor,
  character-alias, and previously inferred local-name shapes.
- `crates/arcweft-tooling/src/speaker_presets.rs:643-684` recursively classifies
  expression forms locally instead of consuming semantic binding identity.
- `crates/arcweft-lang-sema/src/types.rs:350-380` already owns the semantic
  `SpeakerPreset` type family.

The observable failure case is a valid helper return:

```arcw
fn factory() -> SpeakerPreset<Character> {
  SpeakerPreset.new(@character.alice)
}

flow main {
  let preset = factory()
  preset: hello
}
```

The callee spelling `factory` contains no preset clue. The local may therefore
be omitted from tooling's inferred preset set, and canonicalization can expand
the speaker line as a non-preset call. Qualified imports, aliases, new
expression variants, and shadowing create the same class of failure.

## Established substrate that must be preserved

- `arcweft-lang-sema::TypeKind::SpeakerPreset(EntityKind)` is the semantic
  owner. Its identity is not a callee-name convention.
- The parser owns typed speaker-line surface ranges. The tooling fixes for
  AW-AH-001 and AW-AH-002 already removed raw-line `parent::` and colon
  inference; do not replace those ranges or reopen raw source scanning.
- `SourceEditOverlay`, checked UTF-8/range handling, and structured
  `ToolingError` propagation implement AW-AH-004. Canonicalization must remain
  fallible and must not return unchanged source on an edit failure.
- `arcweft-tooling` is Sans I/O and produces deterministic edit plans. Project
  loading, file reads, and LSP transport stay in their existing adapters.
- Parse-only formatting and semantically destructive sugar expansion are
  distinct operations even if the current `FormatOptions` API exposes both.
  Do not force an ordinary whitespace-preserving formatter to invent semantic
  facts.

Do not redesign these implemented fixes without a concrete defect in the
current checkout.

## Design objective

Define one typed semantic input by which formatter/code-action sugar
canonicalization can decide whether a source binding or expression has
`SpeakerPreset` type. The design must handle helper returns, aliases, branches,
and shadowing without teaching tooling another expression evaluator.

## Required design decisions

1. Separate the operations that are syntax-only from operations that require
   semantic identity. State explicitly whether `format_source`,
   `expand_sugar`, the LSP code action, and any CLI canonicalization command
   require a checked project snapshot.
2. Specify the sema-owned result consumed by tooling. It must identify the
   source document, lexical binding/symbol, source range or stable syntax ID,
   resolved `TypeKind`, and scope needed to distinguish shadowed names.
3. Decide whether tooling needs an expression-type map, a binding-type map, a
   checked canonicalization inventory, or a narrower speaker-line resolution
   record. Avoid exporting the full checker implementation merely for
   convenience.
4. Put the `SpeakerPreset` classification behavior on the original semantic
   type, for example an inherent method that can also check the expected
   `EntityKind`. Do not add a tooling extension trait or a second name table.
5. Define how project/module identity, qualified imports, character aliases,
   generic/helper return types, and function results are represented. Tooling
   must not reconstruct any of them from source labels.
6. Define behavior for block, `if`, `if let`, and `match` results, closures or
   helpers, nested flow scopes, reassignment if supported, and same-spelling
   non-preset symbols.
7. Define the stale-snapshot contract. A semantic map must be tied to the exact
   source revision/hash or rejected before edits are generated.
8. Define the unavailable/failed-sema behavior for standalone formatting and
   LSP. It may return a structured unavailable/incomplete result or omit only
   the semantic rewrite, but it must never fall back to the old spelling
   heuristic while claiming canonical output.
9. Define whether partially erroneous programs may receive safe edits. If so,
   the design must state the proof needed for each edited speaker line and how
   unresolved lines are reported without changing them.
10. Define dependency direction and API placement. The intended direction is
    syntax/HIR into sema, then a narrow sema-owned result into tooling/LSP; a
    low-level syntax or sema crate must not depend on LSP or CLI.
11. Define deterministic diagnostic ordering and source ranges when the
    canonicalizer cannot classify a speaker expression, receives a stale map,
    or sees a type inconsistent with the parsed speaker surface.
12. Define incremental/LSP caching and invalidation sufficiently to prevent a
    semantic request on every token edit from becoming an unbounded cost.

## Ownership and layer constraints

- `arcweft-lang-sema` owns type and symbol identity and all
  `SpeakerPreset` classification.
- `arcweft-tooling` owns edit planning from typed ranges plus the supplied
  semantic inventory. It must not own type inference.
- `arcweft-lsp` and CLI/project adapters may obtain a checked project snapshot
  and pass the narrow result to tooling; they must not duplicate classification.
- Parser/CST continues to own lossless ranges and recovery. Sema does not need
  to reopen source text to locate the speaker colon.
- No layer may use `TypeKind::source_label`, `Debug`, or callee spelling as
  nominal identity.

## Non-goals

- Do not redesign speaker-line grammar, dialogue runtime semantics, character
  manifests, or the already checked CST ranges.
- Do not redesign ordinary parse-only formatting beyond separating its API
  from semantically checked canonicalization.
- Do not add callee allowlists, constructor aliases, extension traits, or a
  larger handwritten AST walk.
- Do not preserve the current heuristic as a fallback or compatibility mode.
- Do not change RichText attribute validation or presentation commands in this
  request; those have independent design requests.

## Migration order

1. Specify the syntax-only versus sema-required operation matrix and the
   checked semantic inventory with revision identity.
2. Add the minimal sema-owned query/result and inherent `TypeKind` behavior.
3. Wire the current project/LSP compilation snapshot to the new tooling entry
   point and add stale/unavailable diagnostics.
4. Switch speaker-line canonicalization to the semantic result in one coherent
   cut.
5. Delete `collect_speaker_preset_locals_from_typed_tree`, callee-spelling
   recognition, local expression recursion, and any heuristic-only tests.
6. Remove or replace the old public formatting entry point if it can request
   semantic expansion without semantic input. Do not leave two readers.

No deprecated overload, dual canonicalizer, environment-variable escape hatch,
or migration shim may remain. This language/tooling surface is unreleased
unless concrete release evidence is supplied.

## Diagnostics, errors, and codecs

The design output must define typed error/diagnostic kinds for at least:

- semantic data unavailable for a requested semantic rewrite;
- source revision mismatch or stale semantic inventory;
- unresolved or erroneous speaker expression;
- resolved non-`SpeakerPreset` expression where preset-only sugar is used;
- invalid or overlapping edit application, preserving the existing
  `ToolingError` behavior.

Each diagnostic needs a stable code, primary tight range, message arguments,
and whether formatting stops or returns a partial edit report. A missing
semantic result must not be reported as a parse error and must not silently
select the non-preset expansion.

No new serialized codec is required merely to pass an in-process semantic map.
If the design chooses a persisted incremental representation, it must specify
its version, source hash, limits, decode errors, and rejection of stale or
tampered binding/range references. Do not serialize Rust debug labels as ABI.

## Required tests

- Direct `SpeakerPreset.new` and character aliases canonicalize correctly.
- A helper or qualified imported function returning `SpeakerPreset` works even
  when its name has no preset-related spelling.
- Block, `if`, `if let`, and `match` results preserve the semantic type.
- Chained presets and preset-returning helpers remain correctly classified.
- Shadowing a preset local with a non-preset value changes only the inner
  scope; a same-spelling symbol in another module does not collide.
- A non-preset function or variable whose name resembles `SpeakerPreset` does
  not receive the preset rewrite.
- Stale source/hash data produces the selected structured error and no edits.
- Unavailable or failed sema follows the documented policy without invoking a
  heuristic fallback.
- Unicode identifiers/ranges and CRLF retain exact checked edit boundaries.
- Parser recovery plus partial semantic errors never corrupt unrelated source.
- CLI and LSP use the same sema-backed result and produce equivalent edits and
  diagnostics.
- If a persisted cache is designed, round-trip and tampered binding/range/hash
  cases are tested through its codec API.
- Keep a small source corpus for direct/helper/import/branch/shadowing cases so
  CLI and LSP run the same inputs. Native/Web/headless renderer parity is not a
  boundary for this source-edit-only change and must not be added as fake
  evidence.

Tests must exercise public or crate-owned typed APIs. Do not add a source gate
that searches implementation files for helper names, type spellings, or paths.

## Expected output

- A normative operation matrix for syntax-only formatting and sema-backed
  canonicalization.
- Exact sema result types, ownership, dependency direction, and revision
  identity.
- Resolution examples for direct constructors, helper returns, imports,
  branches, and shadowing.
- Structured diagnostic and partial-result policy.
- An implementation/migration sequence that deletes the heuristic in the same
  series and leaves no compatibility path.
- A focused test matrix covering tooling, sema, CLI, and LSP.

## Acceptance criteria

The design is implementation-ready only when an implementation agent can wire
one semantic query through tooling without guessing API ownership, failure
behavior, or stale-state handling; every audit failure case has a specified
result; and the final state contains no callee-name or AST-shape inference for
`SpeakerPreset` canonicalization.
