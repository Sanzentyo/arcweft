# Compiler diagnostic fixture refresh — 2026-09-09

Inspected base: `da3176436f3b19d1f22cedfd5d67efbe6ca73827`, existing `main`,
pushed and clean before this cut. Supersedes only the diagnostic-fixture
blockers in the [Flow effect record](2026-09-09-flow-effect-publication.md).

Five `.stderr` fixtures now match the observed Rust 1.98.1 diagnostics. Four
compiler fixtures update unresolved-import underline and annotation placement.
The runtime-plan mark-handler fixture updates the similar-name suggestion from
`RuntimeDialogueMarkId` to `RuntimeDialogueMarkFact`. Every intended missing
import/field/method rejection and its error code remains present. No production
Rust, test input, public API, dependency, compatibility path, or contract version
changed. All five diffs were inspected before accepting the regenerated output.

Logs and result JSON are under
`.arcweft-local/validation/2026-09-09-ui-diagnostic-format/`.

| Command | Result |
| --- | --- |
| `TRYBUILD=overwrite cargo test -p arcweft-compiler -p arcweft-runtime-plan --test api_compile --quiet` | Regenerated the five expected outputs; passed, 7.72 seconds. The environment override was scoped to this command and restored afterward. |
| `cargo test -p arcweft-compiler -p arcweft-runtime-plan --test api_compile --quiet` | Passed without overwrite: 4 test functions covering 13 compile-fail fixtures; 7.00 seconds. |
| `just test-workspace` | 851 passed / 18 failed across 83 reports; 36.66 seconds. The compiler API test passes; the recipe now stops at `arcweft-compiler --test callable_execution` (53 passed / the same 18 known native/AWBC failures). |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking` | Passed: 95 packages, 2,239 Rust files, 309 review triggers, zero blocking violations; 2.32 seconds. Non-writing screening. |
| Whitespace and documentation links | `git diff --check` passed; changed relative document links resolve. |

This expected-output-only change adds no structural trigger or state/dependency
boundary. The [Flow ownership review](2026-09-09-flow-effect-publication.md#ownership-review)
remains applicable to the unchanged Rust source. Workspace check, Clippy,
doctests, and Tier 2 were not repeated for diagnostic text alone; their results
and the MCP pattern-binding failure remain recorded in the preceding cut.

Later workspace targets and the CLI recipe commands were not run after the
callable execution failure. The contextual constructor, inferred callback row,
generic continuation, CharacterDialogue value, MCP type/ABI, and subsequent
convergence work remain required. No implementation failure was converted into
an accepted diagnostic snapshot.
