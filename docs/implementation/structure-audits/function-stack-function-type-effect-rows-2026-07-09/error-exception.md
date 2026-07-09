# Structure Audit Error Exception - Function Type Effect Rows

Audit command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-function-type-effect-rows-2026-07-09
```

Result:

- 1 error / 152 warnings.
- Error: `crates/arcweft-lang-sema/src/checker/expr.rs` has 2510 physical LOC,
  exceeding the production-file error threshold by 10 LOC.

Rationale for not splitting in this slice:

- This slice changes semantic function-type representation and effect-row
  propagation; it does not edit `checker/expr.rs`.
- `checker/expr.rs` already delegates substantial behavior to child modules
  such as `callable`, `closure`, `partial`, `pipe`, and `signature_call`.
- Splitting the remaining expression dispatcher in the same commit would mix a
  structural refactor with a semantic type-model change and make validation
  harder to read.

Required follow-up:

- Split `checker/expr.rs` by cohesive expression-dispatch responsibilities in a
  separate structure slice before adding more expression-checking behavior to
  that file.
- Candidate boundaries include literal/path dispatch, call/select dispatch,
  control-expression checking, and statement/block expression checking.
