# Validation report

Date: 2026-08-22

Inspected Git commit:
`61779d1432b902efc2d19041a7326f3c1319828a`

Baseline before design edits: clean; `main == origin/main`.

Status: `READY_FOR_IMPLEMENTATION`

## Performed and passed

From this design directory:

```bash
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../..
```

Result:

```text
PASS sequence=Lang-01.5.1.1.2.1.1.1.1.1.1.1.1
head=61779d1432b902efc2d19041a7326f3c1319828a
model=PASS schema_ast=PASS repository_ast=PASS cargo_graph=PASS
```

```bash
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../.. --self-test
```

Result:

```text
PASS sequence=Lang-01.5.1.1.2.1.1.1.1.1.1.1.1
head=61779d1432b902efc2d19041a7326f3c1319828a
model=PASS schema_ast=PASS repository_ast=PASS cargo_graph=PASS
negative_self_tests=321/321 PASS
```

The repository run checked the recorded production and frozen-predecessor Git
blobs, parsed current Rust ASTs for DenseSeq, Option, AgentBuiltin/live Agent
carriers, the 38 HIR expression families, current function snapshot fields,
and current callable-catalog owner, and used `cargo metadata` to verify that
HIR depends on neither core nor sema. The design-schema run parsed fields, type
definitions, method visibility, adapter prepare return type, keyed maps,
DenseSeq reuse, HIR-only payloads, and every selected zero-valid/zero-invalid
newtype group. Its fourteenth negative self-test mutates `GenerationId(u64)`
back to `GenerationId(NonZeroU64)` and requires `ZERO101`. Its fifteenth
reintroduces `NeedProducerSpecError::{ZeroContract, ZeroPlan,
ZeroPayloadType}` and requires `ZERO105`. The schema checks the infallible
`NeedProducerSpec::new`, all-value semantic digest APIs, public Join/priority/
Host scalar constructors, journal-only allocator values, derived-only fixed
identities, and the generation-mismatch error name. It also verifies every
selected core/scheduler/adapter/HIR constructor and getter, exact private (not
`pub(crate)`) protocol fields, all adapter methods, the sealed core journal
apply, and the four rollback/apply/commit scheduler coordinators. Dynamic
negative fixtures remove every required method, expose every protected type,
remove every coordinator/adapter seam, and add a raw committed-row constructor.

The script was formatted successfully with:

```bash
rustfmt +nightly tools/validate_design.rs
```

A read-only local Markdown link check passed for all 12 edited/new Markdown
files:

```text
PASS local_markdown_links=12
```

`git diff --check` passed for tracked edits. Per-file `git diff --no-index
--check` also passed for all 13 untracked design files:

```text
PASS untracked_diff_check=13
```

## Failed and corrected invocation

The first validator invocation placed an extra `--` before
`--repository-root`. Cargo launched the script, but Clap rejected the argument
and printed usage. The README commands were corrected to the working Cargo
script syntax, and both ordinary and self-test runs above passed. This was a
command-line invocation error, not a contract-validation failure.

An attempted `cargo +nightly clippy -Zscript tools/validate_design.rs -- -D
warnings` invocation was also rejected by Cargo because the `-Zscript` mode
does not expose the script path through the `cargo clippy` subcommand. This was
not a Clippy finding. The script was compiled warning-free by both successful
validator runs; a standalone Clippy result is intentionally not claimed.

## Intentionally not run

No production Rust, Cargo manifest, fixture, codec, generated artifact, or
runtime behavior changed. Workspace check, Clippy, workspace tests, Tier 2,
and structural audit were therefore not run for this design-only cut. They are
mandatory at the production cuts identified in
`CUTS_TESTS_AND_DELETION.md`.

No commit or push was performed by the Sol-max design worker. The primary
workflow integrates the validated design at the reviewable cut.
