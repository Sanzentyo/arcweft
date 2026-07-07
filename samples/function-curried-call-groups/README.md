# function-curried-call-groups sample

This sample demonstrates curried function declaration call groups without
flattening call arguments across groups.

- `tuple_tail(a, b)(c) -> (i64, i64, i64)` keeps the first call group as
  `(a, b)` and applies `c` in a second call.
- `chain(a)(b)(c, d) -> i64` keeps three staged call groups.
- The flow returns the computed sum after typechecking both staged call shapes.

## Check

```bash
cargo run -p arcweft-cli --all-features -- check samples/function-curried-call-groups/src/main.arcw
```
