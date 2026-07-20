# Same-line View modifier-chain parser correction

Date: 2026-07-20

Baseline: `main` `671ebe1738b2`

## Outcome

The View parser now preserves every modifier in a same-line chain such as:

```arcw
Text(line.speaker).x(48px).y(416px).width(860px).height(32px)
```

The previous normalization split only at the first textual `).`. The remainder
was then presented to the first modifier as one malformed argument. The
replacement performs one lossless CST-token scan and splits at every separator
immediately following a completed top-level parenthesized group.

The scan does not treat string or comment contents as punctuation, does not
split nested calls such as `resolve(line).speaker`, and deliberately does not
split selectors following indexed or record values. Each normalized
`ViewSourceLine` retains its original byte start and end.

## Direct evidence

- Production dialogue View properties `x`, `y`, `width`, and `height` remain in
  authored order for both `Text` and `RichText`.
- A nested `resolve(line).speaker` source retains the exact `resolve(line)`
  call range.
- Modifier-like `).name(` text inside a string remains literal content.
- CST tests cover string, comment, index, and record boundaries.
- The CLI bundle regression
  `custom_dialogue_view_role_lowers_and_evaluates_through_the_bundle_runtime`
  passes through the production parser and bundle path.

## Structural measurement

Measured from the current checkout before the final push:

| Path | Owner / role | Bytes | Physical LOC |
|---|---|---:|---:|
| `crates/arcweft-lang-syntax/src/cst/punctuation.rs` | production CST punctuation queries | 23,233 | 735 |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | production View surface parser | 62,448 | 1,824 |
| `crates/arcweft-lang-syntax/src/tests/cst.rs` | crate unit tests | 19,254 | 563 |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | integration tests | 37,696 | 1,361 |

`parser/view.rs` remains above the 1,200-LOC review threshold. This correction
adds one small normalization consumer and does not add another parser,
expression implementation, or semantic responsibility. The canonical
structure audit reports no error-level ownership violation; broader View parser
decomposition remains an independent maintainability concern rather than a
condition of this focused correction.

## Validation

Completed:

- focused same-line View regressions: 4 passed;
- focused CST punctuation regression: passed;
- `cargo test -p arcweft-lang-syntax --all-targets`;
- exact production CLI bundle regression;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  (`0` errors, `128` warnings).

`just test-workspace` progressed through the workspace and failed only in two
pre-existing `arcweft-tooling` tests:

- `canonical_rich_text_visits_statement_bodies_outside_flows`;
- `agent_format_preserves_comments_trivia_and_item_golden`.

Both failures were reproduced on the unmodified `main` baseline before this
cut. The first retains an obsolete `.say()` fixture and the second retains a
stale formatter fixture. They are corrected by the independently isolated
regular-project root-statement removal cut; this View parser cut does not alter
tooling.

`just test-tier2` also passed. It covered the MCP stdio/Agent observe matrix,
animated-image metadata and pixels, mask/object-ID/typewriter/ruby captures,
visual-smoke metadata, and all four checked-in vertical-text IMQ goldens.
