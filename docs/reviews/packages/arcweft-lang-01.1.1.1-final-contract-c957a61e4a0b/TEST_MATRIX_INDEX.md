# Test matrix index

OPEN_QUESTIONS=0

`TEST_MATRIX.csv` is normative. It contains 132 rows.

| Prefix | Rows | Scope |
|---|---:|---|
| SYN | 27 | typed AST/CST, ranges, grouping, trivia, UTF-8, base offsets |
| REC | 11 | malformed and zero-width ordinary recovery |
| HIR | 7 | source retention and single callable identity/catalog |
| SEM | 20 | Result/Option/Await semantics, generics, conversion policy |
| BND | 18 | nearest lexical boundary and generator barriers |
| DIA | 14 | stable codes, typed payloads, primary/related source evidence |
| FMT | 8 | parse-format-parse rules (conditional only because no current formatter owner was found) |
| TOOL | 7 | runtime-plan, verifier, Agent, CLI, dialogue consumers |
| NOC | 7 | no shim, dual reader, source recovery, source gate, CSS/Takumi |
| DOC | 2 | stable design and implementation evidence docs |
| VAL | 11 | focused, workspace, Clippy, doc, audit, and Tier 2 gates |

Rows marked `yes` are mandatory in this correction. `conditional-current-owner` formatter rows are fully specified and become executable only if a production expression formatter owner exists at implementation time; absence of that substrate does not authorize a new broad formatter system or a spelling rewrite.
