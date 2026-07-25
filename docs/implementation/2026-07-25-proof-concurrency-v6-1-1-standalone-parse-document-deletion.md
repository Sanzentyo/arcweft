# Proof-concurrency v6.1.1 standalone `parse_document` deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`

This cut deletes the redundant string-owned `parser::parse_document` public
entrypoint before the larger Stage 3 syntax authority switch. It is a direct
deletion, not a deprecation or compatibility migration.

## Contract decision

The accepted Proof-concurrency v6.1.1 package requires one attached,
caller-owned document authority at the final public switch. The standalone
`parse_document(source, options)` function only forwarded to the same detached
parser already exposed by `parse_source(source)`. It had no production caller
outside `arcweft-lang-syntax` and preserved no source identity unavailable from
another entrypoint.

The older request
`docs/reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md`
mentions `parse_document` while asking for its final migration decision. That
mention is design history, not a compatibility requirement. Reintroducing an
alias would conflict with the accepted package and the repository-wide
deletion policy.

`parse_document_with_source(Arc<SourceDocument>, ParseOptions)` remains because
it preserves the caller's exact document identity and is still used by
compiler and LSP paths pending the atomic Stage 3 switch.

## Deleted surface

- deleted `fragment::parse_document`;
- deleted its `parser` facade re-export;
- changed `parse_source` to call the private full-document grammar directly;
- changed the provisional Items fragment path to call that grammar directly;
- deleted the duplicate entrypoint parity test and duplicate removed-role
  matrix row; and
- updated Agent Script implementation state to name the remaining entrypoints.

No replacement wrapper, deprecated symbol, alias, dual reader, or removed-name
diagnostic was added.

## Direct evidence

```text
rg -n "\bparse_document\b" . --glob '!target/**' --glob '!docs/reviews/**/*.zip'
  PASS: no production Rust occurrence; only this implementation history and
  one historical design request remain
cargo check -p arcweft-lang-syntax --all-targets
  PASS
cargo test -p arcweft-lang-syntax --test removed_role_declarations -- --nocapture
  PASS: 1 passed, 0 failed
cargo test -p arcweft-lang-syntax --all-targets
  PASS: 473 unit tests and all integration/compile-fail tests
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
cargo fmt --all -- --check
  PASS
git diff --check
  PASS
```

The first `just test-workspace` invocation was interrupted after 124 seconds by
the command wrapper's timeout, not by a compile or test failure. The same
recipe was rerun with `CARGO_BUILD_JOBS=4`, changing only build parallelism.
It ran for 978.8 seconds and reached the unchanged inherited Proof migration
gate after all preceding suites passed:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

An exact rerun of
`cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
confirmed 3 passed and only those 2 failed. They are identical to the gate
recorded before this deletion. This cut does not restore a detached reader or
add a compatibility alias to make stale migration fixtures pass.

Tier 2 is not applicable because this cut changes no runtime, render, Agent,
MCP, capture, or corresponding transport behavior.

## Structural audit

The audit used parent Git revision `3f10f2f613e2` and working change
`ukwmvnpx`.

```text
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/proof-stage3-standalone-parse-document-deletion-2026-07-25
files scanned: 3670
Rust files: 1936
Rust physical LOC: 906936
package manifests: 94
violations: 0 error(s), 146 warning(s)
```

Reports are retained under
`docs/implementation/structure-audits/proof-stage3-standalone-parse-document-deletion-2026-07-25/`.

| Path | Bytes | Physical LOC | Classification | Responsibility |
|---|---:|---:|---|---|
| `crates/arcweft-lang-syntax/src/parser/fragment.rs` | 10,479 | 348 | production | remaining detached fragment surface pending Stage 3 switch |
| `crates/arcweft-lang-syntax/src/parser.rs` | 25,749 | 775 | production | parser facade and direct full-document grammar owner |
| `crates/arcweft-lang-syntax/tests/removed_role_declarations.rs` | 2,232 | 59 | integration test | ordinary rejection without duplicate alias coverage |

No changed production file exceeds the 1,200-LOC warning threshold. No
dependency, feature, crate boundary, compatibility surface, or parallel reader
was added.

## Remaining boundary

This cut deliberately does not add the final attached fragment API beside the
old detached one. The larger Stage 3 switch must delete the old fragment and
standalone `ParsedSource`/`TypedSyntaxTree` authority in its local working
change, migrate every compiler, project-loader, CLI, LSP, tooling, and Agent
consumer, and publish only the attached syntax owner in the resulting
workspace-compiling cut.
