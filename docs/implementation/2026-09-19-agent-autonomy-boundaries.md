# Agent autonomy and decision boundaries — 2026-09-19

## Scope and precedence

Supersedes: the workflow decisions in
[the earlier instruction refresh](2026-09-19-agent-instruction-refresh.md),
not its historical account of what was inspected or validated.

The user requested a substantial follow-up: remove old-model approval/stop
scaffolding, retain product invariants and genuine authorization boundaries,
and apply the changes through the GitHub connector.

Inspected remote base: `7d429a559155ee85652ef2c7559ffe838a69c4f4`.
Base tree: `88e1efe8e2a2557078b381b57b368e6ba68ba61e`.
Local repository state: unobserved. The validation environment holds scratch
before/after documents, not a repository checkout or a user worktree.

Scope: five maintained `AGENTS.md` files and four workflow, validation, structural,
and review-intake documents, plus this record. The documentation index and
architecture were read for context, not changed. The user-provided Rust skill
was read completely; external installed/project skills and frozen package copies
were not edited. No new always-loaded skill or model setting was introduced.

## Source basis and project decisions

OpenAI's [Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
(2026-09-11) motivates narrow context, revisiting ask-first boundaries, and
persisting past the first implementation. Direct retrieval of the original URL
failed in this run; its official localized article was retrieved through indexed
[French](https://developers.openai.com/fr-FR/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
and [Spanish](https://developers.openai.com/es-419/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
versions. Supplemental [model guidance](https://developers.openai.com/api/docs/guides/latest-model)
was also consulted for explicit instruction priority and proportional testing.

The concrete choices below are Arcweft policy decisions based on the user's
request, not promises about model reliability or permissions inferred from a
model name. The root design-invariants section is retained unchanged.

| Previous boundary | New decision |
| --- | --- |
| Generic conflicting authority could stop work | Resolve routine technical gaps and precedence from evidence; ask only for consequential unavailable facts, intent, or authorization |
| Skills/process advice could silently veto the task | Current explicit user direction takes precedence over workflow guidance, within higher-priority instructions and tool permissions |
| Incomplete design could become another request | An implementation assignment includes necessary design decisions and the complete consumer migration |
| Initial patch, commit, or compaction invited handoff | Continue the established goal and mode through observable acceptance |
| Every normal Rust push implied workspace tests | Select by affected behavior; isolated changes use their owner/consumer closure, shared integration uses workspace gates |
| Broad runtime/cross-crate label implied exhaustive Tier 2 | Run affected families; exhaustive testing follows actual shared impact or explicit milestone acceptance |
| Every Rust push implied structural scanning | Audit material ownership/dependency changes; use scoped review and reusable dispositions for size/growth triggers |
| Each package resume/push prompted an inventory | Intake the active/dependent or newly changed packages when relevant evidence changes |
| One ZIP allowed only one implementation worker | Keep one integration owner; allow supported independent analysis/non-overlapping work without extra Git permissions |
| Every task could accumulate state paperwork | Keep one useful durable note when acceptance, decisions, intake, or handoff needs it |

Product invariants remain: layer direction, Sans I/O, complete typed ownership,
owner-local behavior, removal of obsolete unreleased paths, contract version `1`,
unsafe invariants, and behavioral rather than source-spelling acceptance.
The Rust skill full-read requirement is explicit again; it is not a demand to
load unrelated reference libraries. Cargo job-count restrictions are unchanged.

Git stays on the existing `main` workflow unless the user requests the alternate
operation. User changes, explicit holds, reserved approvals, and external/system
authorization are preserved. A tool or environment limitation is not converted
into a claimed pass, a reduced contract, or permission to publish production WIP.

## Policy review scenarios

These are manual document-policy reviews, not executed model evaluations:

| Scenario | Expected outcome under the revised policy |
| --- | --- |
| Isolated Rust fix with existing coverage | Fix, run owner checks/lints and meaningful tests, commit/push; no automatic workspace/Tier 2/audit loop |
| Shared codec or language authority change | Complete producer/consumer migration and workspace/contract evidence; do not stop at the first compiling subset |
| Missing design detail with adequate repository evidence | Choose and record the technical solution; no new request or approval round |
| Stale package conflicts with a later accepted contract | Resolve precedence and current adjudication outside the immutable package |
| Continuation of a design-only task | Finish the design/artifact, not production edits |
| Requested ZIP without a compiler | Deliver the archive with limitations inside; no invented readiness or runtime-test result |
| Dirty but reconcilable user edits | Preserve them and continue; ambiguity that would lose user intent blocks only that action |
| User-reserved destructive/external operation | Respect the actual limit, identify it precisely, and continue independent work |
| Repeated push/compaction with unchanged evidence | Reuse recorded passes and resume remaining acceptance; do not rerun by ceremony |
| Completed outcome | Deliver rather than add unrelated fixes, speculative tests, or a new ledger |

## Validation record

Performed and passed:

- Reconstructed all nine edited originals from pinned connector responses and
  matched each byte sequence to its Git blob SHA before editing.
- Reviewed the before/after changes for delegated decisions, actual permission
  boundaries, task-mode preservation, and the scenarios above.
- Ran `python /mnt/data/arcweft-autonomy-refresh/validate.py`: original blob
  identities, changed-file scope, UTF-8/LF/whitespace, Markdown parsing/fences,
  and relative link targets. This is an ephemeral documentation check, not a
  repository source-spelling gate or behavioral acceptance test.
- Ran `git diff --no-index --check` on the scratch before/after trees with no
  whitespace diagnostics. Differences are expected; runtime execution is not
  implied by a scratch diff.

Not run: Rust compilation/tests, Clippy, Tier 2, the structural scanner, model
A/B evaluations, and latency benchmarks. No Rust implementation, executable
recipe, schema, runtime fixture, archive, or dependency is changed. Available
checks establish document consistency, not an empirically measured improvement
in agent behavior. Unobserved local working-tree state and CI are not claimed.

Publication is separate evidence: verify the resulting commit's full changed-file
set and blob identities, parent, and remote `main` after a non-forced update.
This pre-publication note alone does not establish a successful push or CI run.
