# Test profile memory follow-up — 2026-09-09

Inspected base: `d1662e51c936cf9307a354d0b2c8456cfbe168a3`, existing `main`,
verified equal to the remote. The worktree was clean before this change.
The preceding clean-build validation was pushed in
`f645f12a636cde192c90451a391f2042c69d65fe`; its
[record](2026-09-09-convergence-checkpoint.md) retains the exact cold results.

## Problem and selected change

A full `cargo clean` removed 282.4 GiB of generated output, but normal
workspace test compilation still failed. The warm retry reported Windows
error 1455 while mapping existing dependency rlibs, then missing metadata and
delayed rustc internal errors. For that retry the failure was memory commit,
not a missing dependency or exhausted target disk. The 95 workspace packages
have unique package names according to `cargo metadata --no-deps --locked`;
this does not assert that all feature/profile artifact variants are identical.

The root Cargo manifest now sets only `profile.test.debug` to
`line-tables-only`. This retains file/line backtrace information while omitting
type/variable debug information, as defined by the
[Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html#debug).
The rest of the test profile and its inherited development defaults remain
unchanged. The ordinary development profile still provides full debug data;
test-variable debugging can opt into `CARGO_PROFILE_TEST_DEBUG=2` temporarily.
The [test policy](test-execution-policy.md) documents that tradeoff.

No OS paging setting, toolchain version, dependency, feature, Cargo job count,
Rust behavior, or assertion was changed. No second clean was performed.

The two measured workspace-test rlibs decreased as follows on identical Rust
source. This is an artifact-size comparison, not a claim about peak memory:

| Crate | Previous full-debug bytes | Line-table bytes | Reduction |
| --- | ---: | ---: | ---: |
| `arcweft-lang-sema` | 1,600,649,806 | 254,682,718 | 84.1% |
| `arcweft-bundle` | 1,778,593,232 | 970,801,402 | 45.4% |

Exact artifact paths are retained in `artifact-sizes.json` in the validation
directory below. Live compiler command lines confirmed
`-C debuginfo=line-tables-only`. The same-named bundle compiler processes
observed during the run produced a library and a test harness respectively;
they were not two registered workspace packages.

## Validation

Logs, the exact manifest patch/hash, source commit, and results are retained at:

```text
.arcweft-local/validation/2026-09-09-test-profile/
```

- `cargo metadata --no-deps --locked --format-version 1`: passed, 95 members,
  target `D:/git/arcweft/target`.
- `git diff --check`: passed before validation.
- `just test-workspace`: compilation completed without error 1455, mmap,
  missing-rlib, or internal-compiler failures. The command then failed in
  `arcweft-agent-protocol` after 563.99 seconds: 15 completed test reports,
  131 passed / 2 failed / 0 ignored. The two failures were
  `image_resource_metadata_preserves_observed_object_ref` and
  `observation_report_serializes_stable_snake_case_enums`; their old Fx JSON
  fixture lacks the required definition `layout` field. Later test targets
  and the recipe's CLI commands were not executed after this failure.
- `cargo check --workspace --all-targets --all-features`: passed, 17.02 seconds.
- `cargo clippy --workspace --all-targets --all-features`: passed with warnings,
  55.70 seconds.
- `cargo fmt --all -- --check`: passed, 10.64 seconds.
- Three affected documents' relative link targets: 8/8 exist; anchors not checked.

The resource failure no longer blocked this workspace test build. This does
not establish a maximum supported machine workload or a passing whole test
suite. Doctests, Tier 2, and the structural audit were not repeated for this
profile-only follow-up; the preceding cold run records their actual results
on identical Rust source. No dependency/API/ownership structure changed.

The existing sema/callable/Agent failures remain implementation obligations.
Changing debug information cannot establish their correction. The active
convergence goal and its complete execution/restore acceptance are unchanged.
