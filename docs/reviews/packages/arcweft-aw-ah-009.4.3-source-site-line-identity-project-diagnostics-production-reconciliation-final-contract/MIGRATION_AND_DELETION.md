# Direct migration and deletion

## 1. Current owners to replace

### Syntax/HIR source path

- consume the AW-AH-009.4.2 final application/coordinate/source map;
- replace provisional `HirDialogue`/Flow-only line storage with the one HIR
  expression arena;
- make lowering package-aware through `LoweringRequest`/`HirModuleKey`;
- retain typed flow/callable/ScopeId ownership.

### Identity construction

Delete:

```text
DialogueSpeakerSlug
is_narrator_alias line-ID behavior
DialogueLineId<'a> borrowed parser
DialogueIdFamily local string family checks
normalize_line_id(..., speaker, ...)
normalize_line_text_key(..., speaker, ...)
build_line_entity_ref(speaker, ...)
LowerContext.flow_slug for line identity
LowerContext.line_counters: HashMap<String, usize>
.say suffix stripping
callee/entity/display spelling extraction
```

The old helpers are not renamed or wrapped. Durable ID checks move to the
owning lower ID types; owner/prefix behavior moves to the named candidate
builder context.

### Project ownership

Replace:

- `HirProject::new(package: impl Into<String>, modules)`;
- module maps keyed only by `CanonicalModulePath`;
- late callable/package qualification;
- linked/flattened HIR as line-fact authority; and
- any module-local collision-success publication.

with one package-aware `HirProjectBuilder`, package-qualified module map, and
accepted line inventory.

### Diagnostics

Delete line-identity uses of:

- `HirLowerError::new(message, range)`;
- string-only project/typecheck aggregation;
- single-document related-span loss;
- diagnostic message parsing in CLI/LSP/Agent; and
- source-file scans used as completeness evidence.

### Downstream parallel inventories

Delete or never introduce:

- LSP-built line tables;
- compiler-only line maps;
- sema line-ID generation;
- runtime-plan callee-to-line reconstruction;
- tooling source scans;
- Agent observation parsing of line labels; and
- save/replay attempts to infer Character from a line ID.

## 2. Consumer migration inventory

The public replacement must mechanically migrate exhaustive consumers in:

- `arcweft-lang-hir`: model, lower request/context, flow/function lowering,
  project, identity/source maps, tests;
- `arcweft-lang-sema`: checker, project index, source index, reference facts,
  rename inputs, diagnostics;
- `arcweft-compiler`: project assembly, accepted registration, runtime-plan
  lowering inputs, persistent query keys;
- `arcweft-runtime-plan` / verifier: accepted line lookup and checked source-ID
  conversion only; no new wire in this cut;
- `arcweft-project-loader`: package-aware module lowering request and one project
  builder;
- `arcweft-lsp`: accepted snapshot, leases, diagnostics, go-to-definition,
  references, rename, caches;
- `arcweft-tooling`: canonicalization and line/text lookup;
- `arcweft-agent-repl`, Agent/MCP, and CLI: same accepted project generation and
  source diagnostic projection;
- examples/fixtures/docs: direct CharacterDialogue application and frozen line
  identities.

## 3. No compatibility interval

At no compiling frontier may both old speaker-derived and new owner-derived IDs
be successful. Private types may exist unused, but the public switch is direct:

```text
old successful identity construction OFF
new package-aware candidate/project acceptance ON
all consumers migrated
old types/functions deleted
workspace green
```

There is no alias, dual reader, deprecated function, extension trait, string
adapter, versioned source grammar, or old-spelling diagnostic.

## 4. Error-driven migration

After switching the owning enum/types, exhaustive Rust compiler errors identify
call sites. Fix those call sites against inherent methods and named contexts.
Do not add ad hoc matches or `{old}_to_{new}` helpers merely to reduce errors.
When an Arcweft-owned enum needs behavior, add it to that enum's original impl.

## 5. Structural boundary

The change crosses HIR project, diagnostics, semantic indexing, tooling, and
runtime-plan input. Run the canonical structural audit. Keep responsibility
modules focused:

```text
arcweft-id/src/dialogue.rs
arcweft-lang-hir/src/line_identity.rs
arcweft-lang-hir/src/line_identity/candidate.rs
arcweft-lang-hir/src/line_identity/diagnostic.rs
arcweft-lang-hir/src/project/line_acceptance.rs
```

These are suggested responsibility boundaries, not source-placement test
assertions. Final correctness tests use APIs, behavior, codecs, dependency
metadata, and compile-fail contracts rather than file text.

## 6. Completion deletion gate

The implementation is not complete until:

- no production type can construct a line ID from speaker/callee/Character text;
- no generated counter exists outside the module candidate builder;
- no project line inventory exists outside `HirProject`;
- no line diagnostic loses a related SourceSpan;
- no accepted consumer can use another project generation;
- removed public APIs fail to compile;
- ordinary current grammar/method resolution rejects `.say` with no dedicated
  recognizer; and
- all mandated tests and validation pass.
