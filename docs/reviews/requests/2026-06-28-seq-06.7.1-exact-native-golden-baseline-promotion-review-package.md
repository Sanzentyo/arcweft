# Seq-06.7.1 Request: Exact Native Golden Baseline Promotion Review Package

## Sequence Position

This follows the applied seq06.7 exact native golden drift stabilization:

- `docs/implementation/seq-06.7-exact-native-golden-drift-stabilization-2026-06-28.md`

Seq06.7 intentionally did not update checked-in PNG baselines. It defined the
review workflow and required a pinned Windows run with complete evidence before
any baseline promotion decision.

## Problem

The known `vertical_tutr_golden` drift is classified as `baseline_drift`, but
the repository still needs a reviewed evidence packet from a pinned Windows
environment before deciding whether to promote a new PNG baseline or reject the
candidate as renderer/font/environment drift.

## Request

Produce a final zip package containing the pinned baseline-promotion review
packet for exact native goldens.

This package may recommend promotion, rejection, or deferral. It must not hide
the decision inside a blind PNG overwrite. If a PNG promotion is recommended,
put the PNG replacement in an explicit `promotion-overlay/` directory and make
the decision text explain why it is valid.

## Required Inputs

- Current Arcweft `main`.
- `docs/implementation/seq-06.7-exact-native-golden-drift-stabilization-2026-06-28.md`.
- `fixtures/visual-smoke-goldens/exact-native-golden-policy.json`.
- `fixtures/native-golden-drift/vertical_tutr_golden.seq06.6-drift.json`.
- `tests/fixtures/native_capture/`.
- `docs/implementation/test-execution-policy.md`.
- `docs/implementation/fixture-regeneration.md`.
- A Windows environment with the pinned fixture font available and `imq`
  installed.

## Required Run

Run on Windows with:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
```

Then execute:

```bash
just test-visual-golden
just native-visual-artifacts
```

If `just test-visual-golden` fails with `baseline_drift`, that is expected
input for the review, not an automatic package failure. The package must retain
the candidate, observe JSON, imq metrics JSON, and environment fingerprint.

## Required Review Decisions

1. Does the pinned environment match the seq06.7 policy requirements?
2. Is `imq` present, versioned, and running the expected metric set?
3. Are candidate/reference dimensions identical?
4. Are MSE/MAE over threshold because the checked-in PNG is stale, or because
   the environment/backend/font/render path is not acceptable for promotion?
5. Did source fixture text, renderer behavior, font fallback, device scale,
   viewport, or capture code change since the reference PNG?
6. Should the reviewed candidate be promoted to
   `tests/fixtures/native_capture/vertical_tutr_golden.png`?
7. If promoted, are there follow-up changes needed to policy metadata,
   fixture README, or implementation notes?
8. If not promoted, what exact blocker prevents promotion and what run should
   be attempted next?

## Required Zip Output

Return a zip named:

```text
arcweft-seq06.7.1-exact-native-golden-baseline-promotion-review-YYYY-MM-DD.zip
```

The zip must contain:

- `README.md`
- `REQUEST.md`
- `SOURCE_INVENTORY.md`
- `REVIEW.md`
- `DECISION.md`
- `evidence/` with candidate PNG, observe JSON, imq JSON, environment JSON,
  and command logs
- `promotion-overlay/` only if promotion is recommended
- `docs/implementation/seq-06.7.1-exact-native-golden-baseline-promotion-review-YYYY-MM-DD.md`
- `verification/VALIDATION.md`
- `SHA256SUMS.txt`

## Acceptance Criteria

- The review packet is sufficient for a maintainer to inspect the drift without
  rerunning locally.
- Promotion, rejection, or deferral is explicit and evidence-backed.
- If promotion is recommended, the candidate PNG replacement is isolated in
  `promotion-overlay/` and accompanied by post-promotion validation commands.
- If promotion is rejected or deferred, no PNG overlay is included and the next
  run/blocker is concrete.
- Thresholds are not loosened to make the drift pass.

## Post-Promotion Validation If A PNG Is Included

After applying `promotion-overlay/`, run:

```bash
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
just test-visual-golden
git diff --check
```

## Non-Goals

- Changing exact native golden thresholds.
- Moving exact native goldens into default `just test-workspace`.
- Designing CI integration; that is seq06.8.
- Updating unrelated visual smoke fixtures.

## Parallelization Guidance

This review can run in parallel with seq06.8 design. Seq06.8 should not assume
that `vertical_tutr_golden` is promoted; it should consume the seq06.7 status
classes and artifact retention rules either way.
