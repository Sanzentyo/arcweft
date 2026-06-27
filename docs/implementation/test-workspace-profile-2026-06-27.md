# Test Workspace Profile - 2026-06-27

This note records the current `just test-workspace` wall-time breakdown and the
fixture-regeneration command update made from the same investigation.

## Profile Command

Use this entrypoint to reproduce the invocation-level timing:

```bash
just test-workspace-profile
```

The recipe times the same four commands used by `just test-workspace`, plus two
front-loaded probes for the non-CLI workspace slice:

```bash
cargo test --workspace --lib --tests --exclude arcweft-cli --no-run --quiet
cargo test --workspace --lib --tests --exclude arcweft-cli --quiet -- --list
cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
cargo test -p arcweft-cli --lib --bins --quiet
cargo test -p arcweft-cli --test regression_harness --quiet
cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
```

## Initial Measured Breakdown

| Step | Wall time |
| --- | ---: |
| workspace no-run excluding CLI | 34.276s |
| workspace list excluding CLI | 43.491s |
| workspace lib/tests excluding CLI | 137.687s |
| CLI lib/bins | 10.561s |
| CLI regression harness | 4.370s |
| CLI fixture check run | 5.306s |
| `just test-workspace` total | 158.210s |

The full `test-workspace` total is the sum of the four production steps. The
first workspace-wide non-CLI cargo invocation accounts for about 87% of that
time. The `--no-run` and `--list` probes show that the run is not dominated by
linking alone; the broad workspace test execution remains the main cost center.

An attempted crate-by-crate sequential profile was stopped after timing out.
That path forfeits cargo's workspace scheduling and is not a good first-pass
measurement. The better next step is to use `just test-workspace-profile` for
the stable top-level split, then profile suspected crates with focused cargo
commands when a concrete crate changes.

## Warm Recipe Validation

After the refresh work warmed cargo artifacts, `just test-workspace-profile`
also passed with this timing:

| Step | Wall time |
| --- | ---: |
| workspace no-run excluding CLI | 70.445s |
| workspace list excluding CLI | 14.384s |
| workspace lib/tests excluding CLI | 29.856s |
| CLI lib/bins | 45.427s |
| CLI regression harness | 7.761s |
| CLI fixture check run | 4.509s |

The warm run confirms that a single timing is not enough to identify a permanent
hot crate. The durable finding is that `just test-workspace` should be profiled
at recipe-step granularity first, because cargo cache state can move wall time
between compile-heavy and run-heavy steps.

## Fixture Refresh Update

The fixture-refresh inventory found checked-in generated artifacts beyond
`web/demo.awfb`. `just fixture-refresh` now refreshes the portable generated
artifacts:

- `web/demo.awfb`
- WebGPU demo assets under `web/assets/` and `web/.arcweft/asset/generated/`
- JLREQ punctuation generated Rust data

`just fixture-refresh-all` adds native capture PNG candidate generation and the
native fixture-integrity check. Directly overwriting the checked-in native PNGs
was tested during this pass and rejected: the regenerated loose/normal JLREQ
preset PNGs became byte-identical in this environment, causing
`native_checked_in_visual_golden_fixtures_are_well_formed` to fail. The batch
entrypoint therefore writes native candidates under `target/` and leaves golden
promotion as an explicit review step.

## Verification Policy

Use `just fixture-refresh` before ordinary product/web fixture validation. Use
`just fixture-refresh-all` only when native renderer output is under review or
when preparing milestone evidence on the expected Windows setup.
