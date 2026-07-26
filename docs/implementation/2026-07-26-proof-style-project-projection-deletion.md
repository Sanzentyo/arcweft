# Proof module-preserving Style projection deletion

Date: 2026-07-26

Status: `LANDED_VALIDATED_CUT`

Jujutsu change: `qwyzuryspzoyxoxxotonrrspmmryntrm`

Parent main: `e6783fa43b4f` (`Remove flattened HIR from View projection`)

## Result

Compiler Style lowering no longer receives or reads the flattened
`HirProject::linked_module()` projection. `lower_project_view_styles` now
accepts the module-preserving `HirProject`, the checked semantic Style catalog,
and project sources only.

`CheckedPatchInventory` derives the existing project-global inline patch ID
directly from canonical module order and each module-local patch ordinal:

```text
project ordinal = cumulative preceding module patch count + local ordinal
```

This is the same operation previously performed by
`HirModule::append_module_body`, without constructing or reading a flattened
module. Both additions are checked and report `TooManyInlinePatches`; the old
linked/source count-comparison error was deleted. Missing and extra checked
records remain observable through `MissingCheckedInlinePatch` and
`UnreferencedInlinePatches`.

The semantic checker still builds `CheckedViewStyleCatalog` from the current
linked semantic authority. This cut removes only the compiler's flatten-only
projection reader. It does not add a second Style catalog or claim that the
remaining semantic public switch is complete.

## Direct evidence

The multi-module Style test now deliberately supplies project files in
`z -> root -> a` order while asserting canonical `root -> a -> z` patch
identity. The fixture contains two root patches and one patch in each child,
and aligns the first module-local patch range across all three documents. It
proves:

- project patch IDs are exactly `0, 1, 2, 3` in canonical module/source order;
- applications in each owning View resolve to those exact IDs;
- equal local ranges in different modules do not collide;
- every lowered patch declaration's `ViewStyleSourceId -> SourceRangeRef ->
  ProductSourceRef` resolves to the exact owning source-document identity; and
- each product range equals the owning module-local HIR declaration range.

No compatibility wrapper, alias, dual reader, source-string fallback, source
gate, or removed-syntax diagnostic was introduced.

## Validation

Focused and compilation gates:

```text
cargo test -p arcweft-compiler --test style
  PASS: 5 passed
cargo check -p arcweft-compiler --all-targets --all-features
  PASS
cargo clippy -p arcweft-compiler --all-targets --all-features -- -D warnings
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

Normal workspace test:

```text
just test-workspace
  ENVIRONMENT STOP: after 426.3 seconds Cargo reported that arcweft_bundle was
  unavailable in rlib form while building the arcweft-lsp binary.
cargo test -p arcweft-lsp --bin arcweft-lsp --all-features
  PASS: the exact stopped target rebuilt and linked successfully.
CARGO_BUILD_JOBS=2 just test-workspace
  EXTERNAL TIMEOUT: the 603.6-second command timeout closed the trybuild pipe;
  no test assertion had failed at that point.
CARGO_BUILD_JOBS=2 cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
  KNOWN WINDOWS TRYBUILD STOP after the workspace suites: the aggregate
  arcweft-rust-abi-macros compile-fail target stopped at its existing parallel
  fixture-copy race.
cargo test -p arcweft-rust-abi-macros --test compile_fail \
  rejects_unsupported_abi_shapes -- --exact --nocapture
  PASS: 1 passed
```

The remaining CLI recipe stages were executed individually:

```text
arcweft-cli lib/bins                         PASS: 198
runtime_native_options                       PASS: 3
check_core_cli                               PASS: 4
native_style_parity_sample                   PASS: 1
release_trust_json                           PASS: 5
responsive_stage_placement                   PASS: 4
seq04_8_4_persistent_cache_build_cli_goldens PASS: 2
arcw_fixtures_check_run                      KNOWN BASELINE: 3 pass / 2 fail
```

The two unchanged fixture failures are
`spec_should_pass/check/010_capability_fs_read.arcw` and
`spec_should_pass/run/002_file_read_task.arcw`. They remain blocked by the
capability-owned `FsError` nominal publication gap and do not traverse the
Style projection changed here.

Tier 2 passed in full:

```text
just test-tier2
  PASS: MCP/native capture, animated image, text-combine, ruby, visual smoke,
  and exact IMQ golden stages all passed
```

Structural audit:

```text
cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS: 3,682 files, 1,936 Rust files, 906,439 Rust physical LOC,
  94 manifests, 0 errors, 146 repository-wide warnings
```

Exact changed-file measurements at the audit boundary:

| Path | Role | Bytes | Physical LOC |
|---|---|---:|---:|
| `crates/arcweft-compiler/src/project.rs` | production orchestration | 36,661 | 1,091 |
| `crates/arcweft-compiler/src/style.rs` | production Style lowering | 30,597 | 831 |
| `crates/arcweft-compiler/tests/style.rs` | integration test | 18,850 | 528 |

No file crosses a warning or error threshold. `project.rs` remains below the
1,200-LOC production warning, the Style responsibility module remains in the
preferred range, and the integration test remains below its 2,500-LOC warning.
No dependency edge or crate boundary changed.

## Package intake audit

The reviewable cut rechecked all repository ZIPs case-insensitively against
implementation intake records:

```text
repository ZIPs: 29
unrecorded or changed ZIPs: 0
proof v6.1.1.4 final-contract ZIPs: 0
```

The externally supplied Lang-01.1.1.3.1.1 archive remains exactly:

```text
58330347E6759B38770D512BCAA682A1B3949EF46AFF24462F45C23ED851BC63
```

That hash is already verified and recorded by
`2026-07-25-lang-01-1-1-3-1-1-trait-validator-resolver-family-intake.md` as
`READY_FOR_IMPLEMENTATION`; its sidecars are contained inside the ZIP and no
follow-up request is open.

## Remaining boundary

After this cut, no additional production flatten-only HIR reader is safe to
delete before the pending semantic authority contracts:

- Proof v6.1.1.4 must define final expression/leaf/call/dialogue-content
  payloads before compiler, sema, LSP, verifier, runtime-plan, and line-task
  readers can move to the accepted typed project;
- FX needs module-qualified callable/re-export identity rather than a second
  basename linker; and
- the flattened HIR type and `CompiledProject::linked_hir` debug/public
  carriers remain until their semantic consumers and user-visible HIR output
  receive one atomic module-qualified replacement.

Those readers are frozen. This cut does not repair or deepen them and does not
guess the missing Proof v6.1.1.4 schema.
