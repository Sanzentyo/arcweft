# Package validation

## Executed for this archive

- uploaded parent ZIP SHA-256, CRC, path safety, internal manifests;
- all available parent package validators;
- request/evidence/instruction input hashing;
- JSON and CSV parsing;
- required-file and required-token checks;
- absent-symbol normative API checks;
- `OPEN_QUESTIONS.md == "none\n"`;
- final status consistency;
- inventory/test/symbol closure counts;
- executable Python behavioral reference model;
- package manifest member SHA-256;
- ZIP CRC and path safety;
- ZIP member byte parity against the package directory; and
- deterministic rebuild SHA-256 equality.

## Not executed in this design-only environment

- local Git checkout verification through `git rev-parse`;
- `cargo fmt` on production code;
- `cargo check`;
- `cargo clippy`;
- `cargo test` / workspace tests / Tier 2;
- AWBC/native/browser execution parity;
- metadata/structure audit against a local workspace; or
- production patch application.

These are mandatory implementation gates in `IMPLEMENTATION_ORDER.md`, not
open design choices.

## Reproduction

From the extracted package root:

```text
python3 validation/test_reference_model.py
python3 validation/validate_package.py .
```

For the sealed ZIP:

```text
python3 validation/validate_package.py . --zip ../<archive>.zip
```
