# CLI

The CLI should expose syntax-normalization tools without forcing formatter users to give up script-friendly source.

## Check

`arcw check <file.awft>` is the first developer-facing vertical slice. It keeps
file I/O in the CLI adapter and runs the Sans I/O compiler stages over the
source text:

```text
parse_source
lower_to_hir
registry_from_hir
validate_hir_references
lint_id_policy
validate_typecheck_ready
typecheck_hir
lower_line_task_groups
verify_module --mode dev
```

Success prints a compact summary:

```bash
arcw check game/routes/opening.awft
# ok: game/routes/opening.awft (1 flow(s), 1 line task group(s), 0 warning(s), 0 obligation(s))
```

Parse, lowering, reference, readiness, typecheck, and line-plan lowering errors
are reported to stderr and return a non-zero exit code. Syntax lints are printed
as warnings and do not currently fail the command.

## Verify

`arcw verify` is the command-line entry point for the Sans I/O verifier. The
CLI owns filesystem and optional solver process I/O; the verifier core owns
only HIR-to-report analysis.

```bash
arcw verify game/routes/opening.awft --mode test
arcw verify game/routes/opening.awft --mode release --json
arcw verify game/routes/opening.awft --backend oxiz
arcw verify game/routes/opening.awft --backend z3 --solver-command z3
arcw verify game/routes/opening.awft --emit-obligations obligations.json
arcw verify game/routes/opening.awft --emit-smt out/proofs
arcw unsafe game/routes/opening.awft --json
arcw test game/routes/opening.awft --json
arcw bench game/routes/opening.awft --json
```

Modes:

```text
dev      collect obligations and warn on incomplete proofs
test     require formal proof for non-trivial obligations; audited unsafe warns
release  reject missing formal proof and undisclosed audited unsafe
```

Backends:

```text
emit  generate report / SMT-LIB without solving
oxiz  use the pure Rust OxiZ adapter
z3    use the external Z3 process adapter
```

JSON output is shared with LSP and future Agent tooling. Diagnostics include a
stable diagnostic id, obligation id, severity, source span when available,
related ids, and code-action descriptors.

## Test / Bench

`arcw test` and `arcw bench` currently expose the Sans I/O script test manifest.
They parse, lower, resolve, typecheck, and verify the module exactly like
`arcw check`, then list top-level `test` or `bench` declarations. Runtime
execution, renderer/audio driving, clock control, and benchmark timing remain
adapter work.

```bash
arcw test game/routes/opening.awft
arcw test game/routes/opening.awft --json
arcw bench game/routes/opening.awft
arcw bench game/routes/opening.awft --json
```

The manifest preserves the declaration ID, kind, source span, and command-like
body rows so CLI, LSP, headless player adapters, and Agent tooling can share one
planning schema without reparsing source text.

Line-plan lowering must not silently drop parsed syntax. Stable Phase 1.5 cue
syntax such as `at(0.35s): ...` is lowered into Sans I/O line task data.
Line-plan items that are parsed but not yet represented in the Phase 1.5 runtime
model fail `arcw check` with a `LinePlanLowerError` until their lowering is
implemented.

## Syntax Expansion

Default formatting preserves indentation sugar such as `with:`. Expansion is explicit:

```bash
arcw fmt game/routes/opening.awft
arcw fmt --expand-sugar game/routes/opening.awft
arcw fmt --expand-sugar --write game/routes/
```

Expansion rewrites source-level sugar to canonical forms:

```text
with:                 -> with { ... }
speaker: text         -> speaker.say()[text]
speaker(args): text   -> speaker.say(args)[text] for character refs
speaker_preset(args): text
                      -> speaker_preset(args)[text]
await? expr with ...  -> try await expr with ...
parent::path          -> super::path
```

The expansion must preserve the callee kind. A lexical `SpeakerPreset` remains a
callable speaker value, so `alice2(voice=auto): text` expands to
`alice2(voice=auto)[text]`, not to `alice2.say(voice=auto)[text]`.

The command must preserve IDs, source anchors where possible, comments, and stable child entity slots. It must never renumber dialogue or choice IDs as a side effect of formatting.

Relative IDs are not expanded by default because they are author-facing source
syntax. A separate materialization command may rewrite relative IDs to their
fully normalized registry IDs when a project wants explicit IDs in source:

```bash
arcw ids materialize game/routes/opening.awft
arcw ids materialize --write game/routes/
```

Materialization resolves only ID-bearing contexts such as line IDs, text keys,
choice IDs, and choice option IDs. It must not rewrite ordinary entity
references, and it must not invent support for ambiguous forms such as
`goto @.next`.
