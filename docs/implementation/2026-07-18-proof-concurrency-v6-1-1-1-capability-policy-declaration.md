# Proof-concurrency v6.1.1.1 capability-policy declaration

## Source and baseline

- package:
  `arcweft-proof-concurrency-v6.1.1.1-capability-policy-final-contract.zip`
- package SHA-256:
  `bbc5e66e943abb45e85ecec6c14d4be6c5cc4f2fcd899e75c01f0f4848e6dd4d`
- request SHA-256:
  `3a5b332e02240f2c396bd4d30899f4f1ae4227da7e98f906ef9851d52ddb43f5`
- original implementation parent:
  `9a63ac5512cd75947ba70195681e43ab968f9f12`
- current integration parent:
  `69dc5152510d2511dd44481a81d3f283d9f6ae41`
- package decision: `DELETE + DERIVE`

The ZIP was reopened and every entry listed in `CHECKSUMS.sha256` was verified
before implementation. The package contains no overlay or patch manifest;
`MANIFEST.json` is a content/status manifest and records
`implementation_performed = false`.

The pre-change focused capability baseline passed:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax extern_capability
```

On 2026-07-19, Jujutsu change
`yqnnvkvwvxylkztnkotprsvmswzyywvu` was rebased from the original parent onto
current `main` at `69dc5152510d2511dd44481a81d3f283d9f6ae41`. The rebase preserved
the completed Lang-01.2 entry/profile contract and the current typed parser
diagnostic projection. It produced two additive conflicts:

- `crates/arcweft-lang-syntax/tests/public_api.rs` retained both the current
  removed-role/typed-parse-error compile failures and the capability-policy
  absence compile failure.
- `crates/arcweft-lsp/src/diagnostics.rs` retained the current typed
  parser-diagnostic projection tests and added the generic top-level recovery
  regression.

No production ownership or serialized contract conflict was present.

## Final ownership

Arcweft has no source or manifest capability-policy declaration. The source
interface consists only of capability `type` and `fn` members. Each function's
braced effect expressions own that operation's closed external effect set.
Semantic analysis derives reached effects per qualified operation. The exact
selected profile and adapter own target effect availability, and the selected
runtime host owns concrete host-call conformance.

No policy identity, spelling, syntax kind, AST/HIR variant, semantic record,
project/profile field, adapter field, runtime request field, serialized field,
compatibility reader, or schema version is added.

## Implemented acceptance evidence

| Contract area | Implementation evidence |
|---|---|
| Final language documentation | The canonical grammar now contains only `TypeDecl` and `CapabilityFnDecl`, with canonical, multiple-effect, no-effect, availability, and invalid-member examples. The overview grammar agrees. |
| Positive lossless grammar | Empty bodies, documented/attributed/visible members, interleaved types/functions, curried groups, and trailing effect commas retain exact bytes and typed roles. |
| Deleted-shape recovery | Bare, incomplete, unknown, nested-unclosed, duplicate, contradictory, prefixed, and retained-member-tail candidates use existing `ErrorItem`/`ErrorNode` recovery and stable generic codes. Following members and the outer body close remain parseable. |
| Root-context recovery | Policy-shaped top-level braced text uses existing `syntax.parse` / `Item::Raw`, exact expected forms, and ordinary recovery. |
| Public API absence | Trybuild cases prove syntax AST, HIR, sema, resolved profile, runtime template, and runtime request policy APIs do not exist. |
| HIR/sema derivation | Retained capability functions lower with qualified identities. Duplicate effects deduplicate, absent effects stay empty, and calling one operation does not union effects from unused operations. Invalid candidate text creates no HIR/sema policy fact. |
| Selected target ownership | A two-profile/two-adapter topology retains both decoded resources but registers and grants effects from exactly the selected adapter. The other profile deterministically produces `AWF-EFX-007` for the same source. |
| Runtime host ownership | Adapter effect availability does not satisfy a missing host-call implementation; conformance reports the existing exact adapter/call diagnostic. |
| Runtime serialization | Representative template and custom request JSON contain capability, operation, and arguments only. Compile-fail tests reject a policy field. |
| Strict metadata | Injecting a top-level `capability_policy` member is rejected through the ordinary strict unknown-field codec path. Existing hash and duplicate tests remain authoritative. |
| Tooling | Formatting is idempotent for canonical source and byte-preserving for an unknown capability member. LSP publishes ordinary `syntax.parse` recovery for policy-shaped root text without a removed/deprecated diagnostic. |

All evidence is behavioral, codec-based, or compile-fail evidence. No
checked-in source scanner or source gate was added.

## Protected integration boundary

The pinned baseline is still before the proof-concurrency Stage 1 atomic public
syntax switch. The private lossless grammar owns typed capability type/function
members and `ErrorItem` recovery, while the current public
`ExternCapabilityItem` still retains a raw body and the pre-switch public parser
discovers functions from that body.

This split does not bypass that ordered migration:

- it does not expose the crate-private shadow tree early;
- it does not add another raw-body or `signature_tail` reparse;
- it does not partially migrate capability identities into HIR;
- it does not add grammar-aware LSP `type`/`fn` completion or capability-body
  `ErrorItem` diagnostics through a duplicate text classifier.

The full public-AST type-member attachment, canonical public HIR projection,
capability-body LSP diagnostic/completion/hover cases, and any derived
CLI/Agent audit view remain integration-gated on the existing Stage 1 atomic
switch. Their contracts are sufficiently specified here, but implementing them
inside this split would violate the package's protected substrate and ordered
identity migration. No additional design request is required.

## Validation

### Current-main integration validation

The following focused validation was run after rebasing onto
`69dc5152510d2511dd44481a81d3f283d9f6ae41`:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax extern_capability --all-features
cargo test -p arcweft-lang-syntax --test public_api --all-features
cargo test -p arcweft-lang-hir --test capability_policy --all-features
cargo test -p arcweft-lang-hir --test public_api --all-features
cargo test -p arcweft-lang-sema capability --all-features
cargo test -p arcweft-lang-sema --test api_compile \
  capability_policy_absent --all-features
cargo test -p arcweft-lsp \
  policy_shaped_top_level_text_uses_generic_syntax_diagnostics --all-features
cargo test -p arcweft-project-loader \
  selected_profile_owns_one_exact_adapter_effect_inventory --all-features
```

All listed commands pass in the integrated checkout. The first
project-loader run exposed one fixture-only Lang-01.2 drift: both test
profiles omitted the now-required `entry` selector. The fixture was updated
to select `entry.game.main`, and the exact test then passed. No production
code changed for that correction.

The final repository-policy review found no source gate, compatibility shim,
deprecated or dual API, spelling-specific removed-syntax diagnostic, or new
production public surface. It made three test-only corrections:

- the semantic effect test now uses only retained capability grammar rather
  than depending on the public parser's temporary raw-body acceptance;
- the HIR recovery test no longer requires the removed candidate to remain
  diagnostic-free before the later atomic public-parser switch;
- the strict metadata negative test now proves that
  `capability_policy` itself is the unknown field, rather than accepting any
  typed decode failure.

The corrected tests passed:

```bash
cargo test -p arcweft-adapter-metadata \
  capability_policy_is_rejected_as_an_ordinary_unknown_metadata_field \
  --all-features
cargo test -p arcweft-lang-hir --test capability_policy --all-features
cargo test -p arcweft-lang-sema \
  capability_effects_are_qualified_deduplicated_and_operation_local \
  --all-features
```

### Root integration gates

The root checkout is an empty Jujutsu descendant of this proof change and
contains the ignored Noto Sans JP test asset required by the native rendering
tests. The following reviewable-cut gates all passed there:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The canonical audit ran against proof commit
`564110963a107cdf7a589531c0020cb1aaec2576` before this documentation-only
result update. It scanned 3,251 files, including 1,663 Rust files and 759,268
physical Rust LOC, and reported 0 errors and 131 warnings. It wrote no report
files.

The first `just test-workspace` attempt was terminated while its quiet Cargo
process had no visible child process. It had emitted no failed assertion or
diagnostic. The identical first recipe command was then isolated and passed,
and the complete nine-command recipe was rerun with verbose recipe tracing;
it passed in 606.4 seconds. This established that the earlier observation was
runner/output latency rather than a test failure.

Jujutsu reported no unresolved conflicts. A conflict-marker scan and added-line
whitespace scan were also clean.

### Original implementation validation

The final focused validation completed successfully:

```bash
cargo test -p arcweft-lang-syntax extern_capability
cargo test -p arcweft-lang-syntax --test public_api
cargo test -p arcweft-lang-syntax --test parser_declarations_recovery_comments \
  policy_shaped_root_item_uses_generic_recovery
cargo test -p arcweft-lang-hir --test capability_policy
cargo test -p arcweft-lang-hir --test public_api
cargo test -p arcweft-lang-sema capability
cargo test -p arcweft-lang-sema --test api_compile capability_policy_absent
cargo test -p arcweft-project-loader topology
cargo test -p arcweft-project-loader --test public_api
cargo test -p arcweft-adapter-context
cargo test -p arcweft-adapter-metadata
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-runtime-host capabilities
cargo test -p arcweft-core \
  host_request_serialization_contains_only_owned_runtime_fields
cargo test -p arcweft-compiler --test api_compile
cargo test -p arcweft-tooling --test capability_policy
cargo test -p arcweft-lsp
cargo fmt --all -- --check
cargo check \
  -p arcweft-adapter-metadata -p arcweft-compiler -p arcweft-core \
  -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-lang-syntax \
  -p arcweft-lsp -p arcweft-project-loader -p arcweft-runtime-host \
  -p arcweft-tooling --all-targets --all-features
cargo clippy \
  -p arcweft-adapter-metadata -p arcweft-compiler -p arcweft-core \
  -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-lang-syntax \
  -p arcweft-lsp -p arcweft-project-loader -p arcweft-runtime-host \
  -p arcweft-tooling --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The canonical structural audit scanned 3,177 files, including 1,605 Rust
files and 732,713 physical Rust LOC. It reported 0 errors and 129 warnings.
No report files were written. The audit checkout is Jujutsu change
`yqnnvkvwvxylkztnkotprsvmswzyywvu`, based on
`9a63ac5512cd75947ba70195681e43ab968f9f12`.

Cold Windows builds for project-loader, runtime-host, LSP, and nested trybuild
checks exceeded individual command timeouts while their child Cargo processes
were still compiling. Each affected command was rerun from the completed warm
cache and passed; these were build-duration timeouts, not test failures.

The full command below was also attempted:

```bash
cargo check --workspace --all-targets --all-features
```

It reached the existing native rendering tests and then failed because the
isolated Jujutsu workspace does not contain
`web/assets/noto-sans-jp-vf.ttf`. The original checkout has that 9,590,844-byte
file as a `.gitignore`-excluded local asset, and the pinned revision does not
track it. The two existing `include_bytes!` sites were
`arcweft-glyphon/tests/shared_text_layout.rs` and
`arcweft-render-wgpu/src/geometry/dialogue_prepared/tests.rs`. The changed-crate
all-target/all-feature check and warning-denying Clippy commands above both
passed.

### Original implementation structural measurements

The measurements below are the original implementation checkout measurements
at the package parent. They were not relabeled as current after rebasing;
current-main structural measurement remains part of the root integration
audit.

The owning crate is the first path component below `crates/`. Exact original
measurements for every changed Rust file were:

| Path | Classification and responsibility | Bytes | Physical LOC |
|---|---|---:|---:|
| `crates/arcweft-adapter-metadata/tests/codec.rs` | integration test; strict metadata codec | 6,753 | 174 |
| `crates/arcweft-compiler/tests/api_compile.rs` | integration test; trybuild driver | 263 | 6 |
| `crates/arcweft-compiler/tests/ui/runtime_capability_policy_absent.rs` | compile-fail fixture; runtime API absence | 488 | 19 |
| `crates/arcweft-core/src/tests/task.rs` | unit test; task/request codec | 6,052 | 182 |
| `crates/arcweft-lang-hir/tests/capability_policy.rs` | integration test; capability HIR derivation | 2,450 | 81 |
| `crates/arcweft-lang-hir/tests/public_api.rs` | integration test; trybuild driver | 366 | 8 |
| `crates/arcweft-lang-hir/tests/ui/capability_policy_absent.rs` | compile-fail fixture; HIR API absence | 248 | 9 |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | unit test; semantic type/effect analysis | 129,132 | 4,244 |
| `crates/arcweft-lang-sema/tests/api_compile.rs` | integration test; semantic API compile contracts | 890 | 25 |
| `crates/arcweft-lang-sema/tests/ui/capability_policy_absent.rs` | compile-fail fixture; sema API absence | 100 | 5 |
| `crates/arcweft-lang-syntax/src/parser/extern_capability_grammar_tests.rs` | unit test; lossless capability grammar/recovery | 12,309 | 351 |
| `crates/arcweft-lang-syntax/tests/parser_declarations_recovery_comments.rs` | integration test; root recovery | 13,629 | 455 |
| `crates/arcweft-lang-syntax/tests/public_api.rs` | integration test; trybuild driver | 432 | 9 |
| `crates/arcweft-lang-syntax/tests/ui/capability_policy_absent.rs` | compile-fail fixture; syntax API absence | 263 | 10 |
| `crates/arcweft-lsp/src/diagnostics.rs` | production diagnostics plus 449-LOC embedded unit tests starting at line 527 | 36,047 | 975 |
| `crates/arcweft-project-loader/src/topology/tests.rs` | unit test; selected profile/adapter topology | 30,890 | 920 |
| `crates/arcweft-project-loader/tests/public_api.rs` | integration test; trybuild driver | 178 | 5 |
| `crates/arcweft-project-loader/tests/ui/capability_policy_absent.rs` | compile-fail fixture; profile API absence | 162 | 7 |
| `crates/arcweft-runtime-host/src/capabilities.rs` | production runner conformance plus 117-LOC embedded unit tests starting at line 223 | 11,632 | 339 |
| `crates/arcweft-tooling/tests/capability_policy.rs` | integration test; formatter preservation | 1,269 | 40 |

The twenty largest workspace Rust files in the same checkout were:

| Path | Classification and major responsibility | Bytes | Physical LOC |
|---|---|---:|---:|
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | generated Unicode vertical-orientation table | 357,456 | 12,399 |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | integration test; CLI runtime benchmark corpus | 256,505 | 7,970 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | integration test; native vertical-text observation | 238,805 | 6,620 |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | integration test; published JLREQ class mix | 220,473 | 6,109 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | integration test; native sample effects | 214,731 | 5,850 |
| `crates/arcweft-compiler/src/tests.rs` | unit test; compiler pipeline regressions | 180,052 | 5,363 |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | integration test; agent script/debug CLI | 195,821 | 5,249 |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | unit test; semantic type/effect analysis | 129,132 | 4,244 |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | integration test; published JLREQ unit cases | 143,206 | 4,177 |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | unit test; native agent CLI orchestration | 110,752 | 3,181 |
| `crates/arcweft-runtime-driver/tests/session.rs` | integration test; runtime sessions | 106,500 | 2,937 |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | unit test; bundle CLI orchestration | 82,514 | 2,564 |
| `crates/arcweft-core/src/tests/flow.rs` | unit test; core flow execution | 88,953 | 2,553 |
| `crates/arcweft-lsp/src/session/tests.rs` | unit test; LSP session behavior | 85,047 | 2,524 |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | integration test; runtime-plan lowering | 76,729 | 2,521 |
| `crates/arcweft-core/src/value.rs` | production; typed runtime values | 84,017 | 2,500 |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | production; expression type checking | 95,235 | 2,492 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | production; runtime call evaluation | 89,488 | 2,481 |
| `crates/arcweft-runtime-accelerator/src/tests.rs` | unit test; accelerator behavior | 98,746 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | production toolchain profiles plus 296-LOC embedded tests starting at line 2,168 | 75,712 | 2,463 |

The only dependency edit is `trybuild` as a dev dependency of
`arcweft-project-loader`. Its production fan-out remains 15 normal
dependencies; dev fan-out changes from 2 to 3. Workspace fan-in remains the
two normal consumers `arcweft-cli` and `arcweft-lsp`. No production dependency,
feature, crate boundary, or root re-export changed.

## Non-goals and deviations

- No capability member/header/type/function/effect production behavior was
  redesigned.
- No Lang-01.2, live-source, build/profile extraction, View, proof-runtime,
  CSS, or Takumi route was changed.
- No compatibility shim, alias, deprecated node, dual reader, permanent
  removed-spelling diagnostic, raw policy string, or source gate was added.
- The only package-matrix cases not executed in this cut are the public-tooling
  cases gated on the protected Stage 1 atomic syntax switch described above.
  This is an ordered integration dependency, not a replacement policy design
  deviation.
