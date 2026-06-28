# Validation

Date: 2026-06-28
Decision packet: `arcweft-seq06.7.1-exact-native-golden-baseline-promotion-review-2026-06-28`

## Actual validation performed while creating this ZIP

Performed:

```text
- Read local Rust Skill file in full.
- Read current AGENTS.md from GitHub connector.
- Inspected current Arcweft main source inputs through GitHub connector.
- Probed package-generation environment.
- Generated package files.
- Generated SHA256SUMS.txt.
- Verified SHA256SUMS.txt.
- Verified ZIP integrity with unzip -t.
```

Package-generation environment probe:

```text
# local environment probe
2026-06-28T12:40:53Z
Linux ce39398a91ac 4.4.0 #1 SMP Sun Jan 10 15:06:54 PST 2016 x86_64 GNU/Linux
pwd=/
imq=not found
just=not found
cargo=not found
```

Direct local `git clone` did not work in the sandbox because DNS resolution for
github.com was unavailable. Source inspection therefore used the GitHub connector.

Not performed:

```text
just test-visual-golden
just native-visual-artifacts
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
```

Reason: this sandbox was not Windows, did not have `imq`, did not have `just` or
`cargo`, and did not have a local Arcweft checkout.

## Required next validation on Windows

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
just test-visual-golden
just native-visual-artifacts
```

Retain and review:

```text
target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png
target/arcweft-native-capture-artifacts/vertical_tutr_golden.observe.json
target/arcweft-native-capture-artifacts/vertical_tutr_golden.imq.json
target/arcweft-native-capture-artifacts/exact-native-golden.environment.json
```

## If a successor packet recommends promotion

Apply `promotion-overlay/`, then run:

```bash
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
just test-visual-golden
git diff --check
```

No threshold-loosening command is acceptable for making this drift pass.
