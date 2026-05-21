# visual-novel-mini

`visual-novel-mini` is a compact Arcweft project sample. It is intentionally
split into source areas even though each `.arcw` file is currently standalone:
this keeps the layout close to a future multi-file project without depending on
unfinished import/package resolution.

## Layout

```text
visual-novel-mini/
  README.md
  arcw.toml
  src/
    game.arcw        # narrative flow, dialogue, choice, signals, metrics
    server.arcw      # minimal HTTP server entry
    tool.arcw        # minimal CLI entry
  tests/
    opening.arcw     # script test declaration
  benches/
    opening.arcw     # script bench declaration
```

## Useful commands

```bash
cargo run -p arcweft-cli -- check samples/visual-novel-mini/src/game.arcw
cargo run -p arcweft-cli -- check --manifest samples/visual-novel-mini/arcw.toml --profile server.plan
cargo run -p arcweft-cli -- serve samples/visual-novel-mini/src/server.arcw --entry http --adapter native-http --json
cargo run -p arcweft-cli -- serve --manifest samples/visual-novel-mini/arcw.toml --profile server.plan --json
cargo run -p arcweft-cli -- cli samples/visual-novel-mini/src/tool.arcw --entry main --json -- alice
cargo run -p arcweft-cli -- cli --manifest samples/visual-novel-mini/arcw.toml --profile cli.main --json -- alice
cargo run -p arcweft-cli -- test samples/visual-novel-mini/tests/opening.arcw --json
cargo run -p arcweft-cli -- test --manifest samples/visual-novel-mini/arcw.toml --profile test.opening --json
cargo run -p arcweft-cli -- bench samples/visual-novel-mini/benches/opening.arcw --json
cargo run -p arcweft-cli -- bench --manifest samples/visual-novel-mini/arcw.toml --profile bench.opening --json
```

To run the HTTP adapter locally:

```bash
cargo run -p arcweft-cli -- serve samples/visual-novel-mini/src/server.arcw --entry http --adapter native-http --listen 127.0.0.1:8080
cargo run -p arcweft-cli -- serve --manifest samples/visual-novel-mini/arcw.toml --profile server.dev --once
```

The `server.dev` profile includes a listen address. With `--once`, send one HTTP
request from another terminal to complete the process.

## Notes

- `src/game.arcw` uses scenario-style `with:` because it is the formatter's
  preferred human-authored form.
- `src/game.arcw` is currently a checker/tooling sample. Full runtime execution
  of user-defined helper functions remains tied to the launch-profile/runtime
  boundary decision documented under `docs/reviews/requests/`.
- The server and CLI samples are separate files so they can stay Sans I/O at
  the language/runtime boundary and leave host I/O in `arcweft-cli`.
- `arcw.toml` defines launch profiles. Profile mode is the canonical project
  context; direct source paths remain useful for strict source checks and small
  command examples.
- The current sample is deliberately small enough to be used as a regression
  fixture while still showing IDs, entries, dialogue, choice, logging, signals,
  metrics, tests, and benches.
