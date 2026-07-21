# Proof concurrency v6.1.1 — public authority switch readiness

Date: 2026-07-21

## Meaning of the switch

The Proof switch is not a runtime feature flag. It is the atomic authority
change from the detached syntax and clone-based HIR paths to one
revision-bound compiler model:

```text
SourceDocument
  -> ParsedSource + SyntaxDatabaseId/SyntaxLineageId
  -> attached SyntaxNodeId family nodes
  -> arena-backed HIR/project/scope ownership
  -> sema/verifier/runtime assertion identity
  -> strict bundle/save/replay records where the identity persists
```

After the switch, `TypedSyntaxTree`, line-derived identity bridges, duplicate
fragment parsers, cloned/linked HIR assembly, and source-string reparsing are
deleted. There is no dual reader, compatibility wrapper, or detached public
AST retained beside `ParsedSource`.

## Current implemented prerequisite

On Git `a83613775abebb9574e9c8cee549e7ede798574e`, the private predecessor now
includes:

- the one-pass lossless grammar and typed event/kind inventory;
- database, lineage, and attached syntax-node identities with atomic rollback;
- path-authoritative attachment for equal-kind equal-offset recovery nodes;
- a database-bound private parse product; and
- database-bound expression, type, pattern, and ordinary-statement fragment
  families using the shared lexer and grammar transaction.

These cuts intentionally expose no second public reader. They prove the target
identity and recovery behavior while production callers continue to use the
old authority until the final inventory is stable.

## Remaining prerequisite decisions and implementation

The public node inventory must be final before identity becomes public. The
remaining prerequisites are:

1. complete Lang 01.1.1 so `task fn`, `dialogue fn`, and `stream fn` no longer
   remain independent callable kinds where ordinary functions plus typed
   suspension/role metadata are the selected model;
2. complete the implementation-ready Lang 01.3 source elimination portions
   without guessing the still-unreturned Lang 01.3.1.2.1 runtime/wire
   correction;
3. complete the Lang 01.4 public `res` migration for configured resources while
   retaining dedicated stable-identity declarations such as Character, View,
   Action, Activity, Signal, Metric, Layer, and Asset;
4. preserve the already-selected Lang 01.5 and trusted-proof ownership rather
   than adding private nodes for removed `extern mod`, `dialogue defaults`,
   source `content`, concrete Activity origin, or `trusted axiom` syntax.

The former RichText prerequisite is now complete in the
[private attached RichText grammar](2026-07-21-aw-ah-007-008-private-rich-text-grammar.md):
tags, ordered scalar arguments, exact recovery descendants, and dedicated
expression payloads share the private lexer/event/attachment transaction
without publishing a second reader.

The unreturned correction contracts remain explicit non-goals. Their absence
does not justify inventing a wire shape or preserving an obsolete source form.

## Atomic public migration

Once the prerequisite inventory is fixed, the switch proceeds as one coherent
authority migration:

1. make public `ParsedSource` the only complete-document and fragment parse
   product and expose only attached typed nodes;
2. migrate HIR lowering to consume attached nodes and allocate one immutable
   arena database with module, scope, local, capture, predicate, proof, and
   source provenance tables;
3. migrate project construction, symbol publication, sema, verifier, CLI, LSP,
   Agent, MCP, formatter, and runtime-plan callers to the same database
   identity;
4. replace linked/cloned module assembly with module-preserving arena project
   ownership;
5. publish final typed `ProofBlock`, proof artifact identity, trusted-proof
   metadata, and verifier obligations from that HIR authority;
6. bind runtime assertions, faults, diagnostics, codecs, save, and replay to
   the same proof/assertion identity; and
7. delete the detached syntax tree, old line bridge, duplicate parse entry
   points, source reparsers, linked HIR accessors, and compatibility-free old
   codec paths in the same cut.

## Current migration surface

This is a one-off checkout inventory used for planning, not an automated source
gate. At the revision above, Rust consumers are distributed as follows:

| Old boundary | Rust files naming it |
| --- | ---: |
| `.typed_tree()` | 80 |
| `TypedSyntaxTree` | 20 |
| `lower_to_hir` | 65 |
| `linked_module` | 12 |
| `linked_hir` | 7 |
| `ParsedFragmentKind` | 9 |

The counts include tests and may overlap. They demonstrate why a partial public
switch would create two authorities; they are not acceptance criteria and are
not checked by CI. Completion is established through typed API migration,
compile failures at removed boundaries, behavioral tests, strict codec tests,
dependency evidence, and Tier 2 execution.

## Completion evidence required

The switch is complete only when:

- one accepted source revision owns every public syntax and HIR identity;
- no executable proof/assertion path can be built from detached or reparsed
  source;
- public/compiler behavior has no old-reader fallback;
- parser, HIR, project, sema, verifier, runtime assertion, bundle, save/replay,
  LSP, Agent, MCP, and formatter tests pass against the same authority;
- the applicable workspace check, strict Clippy, format check, structural audit,
  and Tier 2 suites pass; and
- the old boundaries above are deleted because their typed callers migrated,
  not because a source-spelling gate says their names disappeared.
