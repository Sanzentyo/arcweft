# Proof convergence: unused public syntax facade closure

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes four unreleased syntax entry points from the
workspace's public authority without adding a replacement wrapper:

- `text::parse_dialogue_tokens` is deleted. It only forwarded to
  `parse_dialogue_text(source).into_tokens()` and had no production consumer.
  The semantic dialogue tests now consume the diagnostic-owning parse product
  directly.
- `parser::parse_dialogue_content` becomes crate-private. Its three active
  consumers are syntax-owned parser/range projection code.
- `cst::text::parse_flat_fence` becomes crate-private. Its seven active
  consumers are syntax-owned dialogue, flow, and line-plan parsers.
- `cst::cst_lines_for_source` becomes crate-private. Its production consumers
  are the current syntax parser; the remaining references are syntax unit
  tests.

Workspace-wide reference discovery found no external consumer of the three
visibility-restricted functions and no production consumer of the deleted
token convenience function. The existing
`removed_unused_syntax_facades.rs` compile-fail row proves that downstream
crates cannot reconstruct these public paths. No source scan, compatibility
alias, deprecated forwarding function, dual reader, or removed-syntax
diagnostic was added.

The implementation is Jujutsu change `lyownsupoysz` over parent Git commit
`ac9ce44fe9423efd85280e26832dd30c725b3b34`.

## Deliberately retained boundaries

The Agent REPL item carrier was independently re-audited before selecting this
cut. `FragmentKind::Items`, `ParsedFragmentKind::Items`, and the private
`parse_source_with_options` cannot be deleted alone without one of the
following prohibited replacements:

- a second compiler entry that accepts a pre-parsed module;
- an optional preparse field and dual compiler reader;
- an Agent-only lowering path; or
- parsing the same executable synthetic document once for classification and
  again in `compile_module`.

The accepted Proof contract requires the exact synthetic `SourceDocument` to
be parsed once, retained by `ParsedReplCell`, and passed as the same bound
`ParsedSource` lease into source-backed lowering. The current compiler accepts
only `ProjectSourceFile` documents and reparses them. The atomic deletion
therefore remains coupled to the corrected
[`01.1.1.4.1` leaf-expression redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
and the public ParsedSource-to-HIR/compiler authority switch. The failed
`NOT_READY` return remains recorded in
[`2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md`](2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md).

The file-local parser helpers in grammar tests were also audited. They are
test-only owners of fixed logical `SourceDocument` identities, not public or
compatibility wrappers, and they return the bound parse product without
discarding its document. Inlining 139 calls would duplicate setup while
removing no production authority, so they remain.

## Validation

Completed:

- `cargo fmt --all`;
- `git diff --check`;
- `cargo test -p arcweft-lang-syntax --test public_api --all-features --
  --nocapture`: all 12 compile-fail rows passed;
- focused semantic dialogue tokenizer tests: eight passed;
- focused typed wait/speed boundary test: one passed;
- `cargo test -p arcweft-lang-syntax -p arcweft-lang-sema --all-targets
  --all-features`: passed, including 1,118 semantic unit tests, 494 syntax unit
  tests, all integration suites, and both crates' compile-fail suites;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- the exact `arcweft-project-loader --test release_trust_e2e` suite: all six
  passed; and
- the repository ZIP inbox ledger: 30 retained archives, every SHA-256 already
  referenced by an implementation intake/completion note, zero unrecorded
  archives, and zero ZIP files left directly under `docs/reviews/`.

The first combined syntax/sema command was given a 120-second process limit.
Its child tests were green up to that point, but the runner timeout closed the
output pipe. The same complete command was rerun with a sufficient limit and
passed; the timeout is not counted as validation evidence.

`just test-workspace` was run twice. The first run stopped at
`release_remote_publish_file_mirror_archive_verifies_after_publication`; that
test then passed alone and its complete six-test integration suite passed. The
second workspace run passed that suite and all preceding suites, then reached
the established `arcweft-cli --test arcw_fixtures_check_run` baseline: three
passed and the same two specification fixtures failed:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both fixtures still require publication of the capability-owned `FsError`
nominal through the attached HIR authority. That gap predates this cut and does
not traverse any changed syntax facade. The exact CLI integration suite was
rerun and reproduced only those two rows. This cut does not add a global
`FsError`, fallback nominal, compatibility reader, or fixture bypass to make
the broad gate superficially green.

Tier 2 is not applicable: this cut changes no runtime, renderer, Agent, MCP,
capture, persistence, or serialized contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-unused-public-syntax-facade-closure-2026-07-27/`](structure-audits/proof-unused-public-syntax-facade-closure-2026-07-27/).
It scanned 3,746 files, including 1,948 Rust files and 906,403 physical Rust
LOC across 95 manifests, and reported zero errors and 146 existing warnings.
The warning-heading inventory is identical to the parent audit.

Changed production owners are:

| Owner | Bytes | Physical LOC | Responsibility |
| --- | ---: | ---: | --- |
| `arcweft-lang-syntax/src/text.rs` | 36,952 | 1,054 | diagnostic-owning dialogue/RichText text parser after wrapper deletion |
| `arcweft-lang-syntax/src/parser.rs` | 25,601 | 773 | syntax parser facade with dialogue-content construction now crate-private |
| `arcweft-lang-syntax/src/cst.rs` | 12,519 | 429 | current lossless CST and crate-private line projection |
| `arcweft-lang-syntax/src/cst/text.rs` | 4,322 | 115 | crate-private flat-fence/source-text helpers |

No changed production file crosses the 1,200-LOC warning threshold, no Cargo
dependency or feature changes, and no new ownership responsibility was added.

## Next boundary

After this slice is validated and pushed, the next independent deletion is the
zero-consumer HIR public surface: private-only lowering namespaces,
`HirProjectModule::into_parts`, and unused `HirModule` accessors. The broad
`arcweft_lang_hir::syntax` forwarding facade can then be removed by migrating
its consumers directly to `arcweft_lang_syntax`, which they already depend on.
Linked/cloned HIR remains frozen until the final project-aware checker owner
can replace it atomically.
