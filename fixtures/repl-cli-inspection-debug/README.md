# REPL CLI inspection/debug fixtures

These fixtures describe the behavior tests expected after applying the patch. They are intentionally small and deterministic so they can be adapted either to Rust unit tests or CLI golden tests.

- `parser-stage-shared-command.json`: shared parser selection.
- `parser-stage-cli-command.json`: CLI parser selection.
- `unknown-command.json`: both-stage unknown diagnostic.
- `malformed-cli-command.json`: CLI malformed-argument diagnostic.
- `read-only-trace-rejection.json`: mutating CLI command rejection.
- `human-output-cli-families.txt`: expected human labels by CLI command family.
- `json-output-cli-families.json`: expected typed JSON evidence kinds.
- `shared-result-preservation.json`: shared `ReplCommandResult` preservation.
