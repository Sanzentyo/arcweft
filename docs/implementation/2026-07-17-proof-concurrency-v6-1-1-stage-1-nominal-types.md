# Proof-concurrency v6.1.1 Stage 1 nominal type grammar

## Scope

This cut extends the private, lossless shadow grammar with the retained
`enum`, `struct`, and `type` declaration families. It is based on main
`e896b5016076` and Jujutsu change `llzwyzty`. It does not switch the public
parser, syntax identity, HIR, project, or runtime contracts.

All three families consume the document parser's existing lexer tokens and
emit into its one cursor/event stream. The implementation introduces no source
substring reparse, compatibility alias, removed-syntax recognizer, source
gate, CSS route, or Takumi route.

## Typed ownership

- each declaration owns its visibility, ordinary name, and optional generic
  parameter group;
- enum variants and struct fields are independently identity-bearing
  `RecordField` nodes under the non-identity `FieldList` wrapper;
- record-variant payload fields own their names and recursively typed payload
  types;
- tuple and nominal enum payloads reuse the shared type grammar;
- type aliases own a typed target and expression-backed `where` predicates;
  and
- real and missing braces use the shared delimiter-node authority.

`RecordField` is the final grammar vocabulary's shared field boundary. The
semantic parent and `SyntaxRole::Field` distinguish enum variants, record
payload fields, and struct fields without adding another provisional syntax
kind.

## Recovery

- a field without `: Type` retains its name, a zero-width `MissingType`, and an
  `ErrorNode` for the unexpected tail;
- a type alias without `=` or a target emits typed missing evidence;
- missing record-payload and declaration braces emit zero-width close nodes;
  and
- an unclosed declaration synchronizes before the following unindented
  declaration, which remains a sibling item.

Every recovery case remains byte-lossless and queryable. This private shadow
output allocates no production `SyntaxNodeId` and is not executable.

## Direct evidence

`parser::type_declaration_grammar_tests` covers the production ADT fixture with
an attributed public enum, unit and record variants, a public struct, and a
type alias with two constraints. It asserts exact typed descendant counts and
byte-for-byte green-tree text. Negative cases cover a malformed field, a
missing alias target, and nested missing braces followed by a clean proof
declaration.

The focused suite passes 3/3. The complete `arcweft-lang-syntax` suite passes
187 unit tests together with every integration, UI, and documentation test.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo fmt --all
cargo test -p arcweft-lang-syntax type_declaration_grammar_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-nominal-types-2026-07-17
```

The first Clippy run rejected an unnecessary raw-string hash in the new test;
the spelling was corrected and the exact Clippy command then passed. The
workspace check completed all 90 packages successfully. After rebasing onto
`e896b5016076`, the focused tests, syntax Clippy, workspace check, and workspace
Clippy were rerun and passed.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-nominal-types-2026-07-17/`.
It scanned 3,072 files, 1,537 Rust files, 706,511 physical Rust LOC, and 90
manifests with zero errors and 130 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 24,003 bytes / 694 physical LOC, production parser facade;
- `parser/document.rs`: 19,492 bytes / 612 physical LOC, private document/event
  orchestration;
- `parser/type_declaration_grammar.rs`: 11,801 bytes / 354 physical LOC,
  production nominal declaration grammar; and
- `parser/type_declaration_grammar_tests.rs`: 3,828 bytes / 121 physical LOC,
  direct test module.

All changed production files remain below repository warning thresholds. This
cut changes no Cargo dependency, feature, public API, serialization contract,
or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining Stage 1 work

Stage 1 remains open. `trait` and `impl` declarations, then the remaining
retained item families, still require complete typed descendants and recovery
coverage. Lang-01 design work continues to own the final role of declarations
whose top-level surface is being removed or unified. Proof-concurrency Stages
2 through 8 remain open.
