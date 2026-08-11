# Repository evidence

## 1. Inspection identity

Repository: `Sanzentyo/arcweft`  
Branch: `main`  
Inspected Git commit: `8984661d5679efccf7a16255f921530cd0b7cacc`  
Commit subject: `Audit AW-AH-009.3 production reconciliation`  
Immediate relevant predecessors:

```text
354cc0964a21ab16b5e039fbf3da96f98584a147  Parse record and block expression families
328e362f811896ebf866002c458fe0b970976654  Parse bracket and call argument expressions
```

Jujutsu change: `unavailable`. The authenticated repository connector exposes Git data but no Jujutsu checkout or change identity.

Inspection date: `2026-07-16`.

## 2. Required instructions inspected

- Root `AGENTS.md` was read in full at current `main` (blob SHA `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758`). Applicable rules include direct replacement for unreleased parser/compiler refactors, private typed ownership, no compatibility shims, explicit source spans/recovery, focused then workspace validation, and the canonical structural audit.
- The supplied Rust skill was read in full. The contract follows its ownership, visibility, exhaustive enum, checked arithmetic, documentation, test, format, and Clippy requirements.
- The supplied Arcweft premise was read in full and current repository philosophy/structure was inspected before selecting the model.

No nested `AGENTS.md` was identified for the inspected crate paths through the available repository tree/search evidence. The root file is the applicable instruction owner recorded here.

## 3. Parent-contract evidence

The original AW-AH-009.3 package bytes were not present in the mounted inputs. The following authoritative inputs were available and inspected:

- governing AW-AH-009.3.1 reconciliation request, which reproduces the contradictory call-syntax clause and freezes the non-redesign boundary;
- AW-AH-009.3 status sidecar with `STATUS=READY_FOR_IMPLEMENTATION`, baseline commit, archive name, and exact SHA-256;
- AW-AH-009.3 summary describing the selected position-aware sema query, exact source-document/range path, one resolver, and unchanged character nominal identity;
- current repository implementation note `docs/implementation/2026-07-16-aw-ah-009-3-signature-help-decided-substrate.md`;
- current production audit `docs/implementation/2026-07-16-aw-ah-009-3-production-reconciliation-audit.md`.

This archive does not claim byte-level inspection of the unavailable parent ZIP members. The result-changing parent clause is fully restated by the governing request, and every other parent policy is inherited unchanged rather than reselected.

## 4. Current source-AST evidence

### 4.1 `crates/arcweft-lang-syntax/src/expr.rs`

Observed at the inspected commit:

- public `Expr` has one struct-like `Call { callee: Box<Expr>, args: Vec<CallArg> }` variant;
- public `Expr::call` and `Expr::selected_call` construct calls without source or ranges;
- `CallArg` distinguishes positional, named, and postfix spread semantic forms;
- closure parameters are typed semantic values with private fields/read accessors.

This proves that a private payload is required to make parser-only call construction enforceable. Public enum fields cannot enforce the invariant.

### 4.2 `crates/arcweft-lang-syntax/src/expr/pratt.rs`

Observed:

- `Token::LParen` parses arguments and calls `Expr::call`;
- selected member calls use the same source-less constructor;
- selected callback blocks lower to `Expr::Call` with one positional closure argument;
- `parse_call_args` consumes punctuation but returns only semantic arguments;
- incomplete `)` handling does not retain a typed argument-list terminator.

This proves that exact ranges are available at the correct parser layer but discarded before AST construction.

### 4.3 `crates/arcweft-lang-syntax/src/expr/closure_parse.rs`

Observed:

- callback blocks parse braces, optional explicit `=>` headers, multiple and typed parameters, nested delimiters, and multi-statement bodies;
- no-arrow blocks represent zero parameters;
- empty explicit parameter headers, empty bodies, and missing closing braces are rejected;
- token spans are available during the parse but discarded from the semantic closure/call result.

This supports a dedicated exact callback carrier and retention of current invalid/unclosed behavior.

### 4.4 `crates/arcweft-lang-syntax/src/expr/source_ranges.rs`

Observed:

- range collection reconstructs expression ranges from source after semantic parsing;
- the call branch can search for postfix parentheses and split source text;
- callback-block applications cannot be represented as a parenthesized argument list by this mechanism.

This supports deleting call-specific source reconstruction and consuming parser-owned syntax instead.

### 4.5 `crates/arcweft-lang-syntax/src/parser/helpers.rs`

Observed:

- `parse_expr_lossy` trims/normalizes source and falls back to `Expr::Raw`;
- `parse_static_generic_call` scans raw source and directly constructs a source-less call;
- speaker/call-head helpers split raw argument strings and locate values after parsing;
- `is_expression_statement_call` directly matches the old variant.

These are explicit migration/deletion points in Cuts 2–4.

## 5. Dialogue and speaker evidence

### 5.1 `crates/arcweft-lang-syntax/src/ast/dialogue.rs`

Observed blob SHA: `95af293a74ade4528ae6c9661a92592b517e448b`.

- `SpeakerLineSurface` stores `arguments: Option<TextRange>` for the trimmed interior only and derives `Copy`;
- its accessor explicitly excludes parentheses;
- `ContentCall` has no dedicated call-head surface;
- `LineOptions` and `LineArg` retain semantic expression values and selected raw/value ranges.

This proves that speaker/content heads need the same owned `ArgumentListSyntax`, and that `SpeakerLineSurface` must cease to be `Copy`.

### 5.2 `crates/arcweft-lang-syntax/src/parser/dialogue.rs`

Observed blob SHA: `c89fc70828c688700555763bbf462dcd6de87eed`.

- colon speaker parsing passes raw argument text/base into `parse_line_options` and stores only the interior range;
- content-call parsing uses `split_call_head` and a tuple `ContentCallParse` carrying optional raw args/base;
- dialogue expression attachment currently invokes a post-parse source-range collector;
- content brackets and trailing plans have separate ownership that must remain intact.

This supports a shared token-level list parser, named parser result, exact special-form surfaces, and no synthetic ordinary call.

### 5.3 `crates/arcweft-lang-syntax/src/parser.rs`

Observed blob SHA: `c3a05346e10ca6c25a42d364434a2d9e961aaa13`.

- `ContentCallParse` is a tuple containing `String`, optional raw args/base, content, end, plan, and trailing scope;
- the full parser owns a `SourceDocument` when parsing a document and retains source/error sinks.

This supports replacing the tuple with a named exact-surface result and routing recovery diagnostics through the existing parser owner.

## 6. Existing behavioral coverage

`crates/arcweft-lang-syntax/tests/parser_callbacks_and_closures.rs` currently covers:

- ordinary named and spread calls;
- callback blocks with zero, one, multiple, and typed parameters;
- multi-statement callback bodies;
- a parenthesized call followed by `.on_click { ... }`.

Current assertions destructure the source-less call variant. These are direct migration targets and demonstrate that callback syntax is retained production grammar, not removable compatibility syntax.

## 7. HIR evidence

`crates/arcweft-lang-hir/src/model.rs` imports and stores `arcweft_lang_syntax::expr::Expr` directly and intentionally remains close to syntax. Its source map retains source-document identity/provenance.

Therefore:

- deriving `Clone` on the final syntax carriers preserves them through current HIR;
- a second HIR call-surface enum is unnecessary and would create a dual representation;
- direct HIR equality/range tests are sufficient for this cut.

## 8. Sema evidence

`crates/arcweft-lang-sema/src/checker/expr.rs` and related checker/fact modules exhaustively match the current struct-like call and consume semantic callee/arguments. They do not require punctuation for ordinary type/effect checking.

Current search inventory exposes source-call matches across checker expression support/partial evaluation/signature/effect/Fx/lifetime, fact layers, project index, and style traversal. The final accessor migration preserves one checker and one semantic call meaning. Only signature-site extraction needs `ParenthesizedCallSyntax`.

## 9. Runtime, verifier, CLI, and tooling evidence

Current search inventory exposes same-spelling call matches across:

- `arcweft-runtime-plan` expression, desugar, effect, pure, labels, trait-method, flow, audio, Fx, render-text, and host-request paths;
- `arcweft-verify` contract lowering;
- CLI runtime expectation and Agent snapshot paths;
- Agent REPL binding;
- core runtime evaluator and other domain-owned IRs.

Not every `Expr::Call` spelling names the source AST. The implementation handoff therefore requires type-owner inspection and migrates only matches whose scrutinee is the syntax/HIR `Expr`. Existing source-independent runtime calls remain the selected generated representation.

## 10. Current audit note evidence

The current repository production audit records the same contradiction:

- normal parenthesized and callback-block calls share the source-less semantic variant;
- public constructors remain range-free;
- post-parse range reconstruction cannot establish the parent contract for callback syntax;
- proof-concurrency changes do not select a source-AST solution.

The inspected head is itself the audit commit, so no later production Rust change was found that resolves the seam.

## 11. Verification actually performed for this delivery

Performed:

- current private repository inspection through the authenticated GitHub connector;
- current head and relevant predecessor identity check;
- full root `AGENTS.md`, full supplied Rust skill, and supplied premise review;
- targeted owner/producer/consumer source review at immutable commit;
- original parent package name/SHA/status verification from governing input and sidecars;
- exact required archive-member audit;
- UTF-8/LF audit for Markdown/text members except the intentionally four-byte `OPEN_QUESTIONS.md`;
- sorted manifest and per-member SHA-256 verification with the 64-zero self-entry rule;
- deterministic ZIP metadata, CRC/integrity, clean extraction, and sidecar/hash agreement checks.

Not performed or claimed:

- no local Arcweft checkout was available in the execution container;
- no Rust toolchain was installed in the execution container;
- no future production patch was created, as prohibited by the request;
- no Cargo format/check/Clippy/test or repository structural audit was run against changed production code;
- no Jujutsu command was available;
- no byte-level read of the unavailable parent ZIP was claimed.

The unrun production commands are implementation gates in `IMPLEMENTATION_HANDOFF.md`, not delivery successes.
