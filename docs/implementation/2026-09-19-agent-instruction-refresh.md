# Agent instruction refresh — 2026-09-19

## Scope and evidence

Requested: reconsider repository skills, prompts, and instructions using OpenAI's
[Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
(2026-09-11), and apply the selected updates through the GitHub connector.

Inspected remote `main`: `827aaedb955b81c21ede5c8deed7cb6d5440a826`.
Base tree: `5f662737188baa8ca70531f2eb2935cca8513cce`.
Local repository working-tree state: unobserved; there is no repository checkout
in this task's validation environment. Scratch Markdown copies are not a checkout.

Reviewed all five indexed `AGENTS.md` files, the documentation index, review
intake, test/structural policies, and `Justfile`/`just/verify.just`. GitHub filename
search returned five `AGENTS.md` files and only one `*SKILL.md` match: the frozen
`inputs/RUST_SKILL.md` inside the retained task-plan semantic-child-encoder
contract package. It is not an active repository skill and was not edited.
User-global/installed skills and external agent configuration are outside this
repository update; no model setting or new always-loaded skill was added.

## Decisions implemented

| Surface | Change | Boundary retained |
| --- | --- | --- |
| Root and four scoped instructions | Replace blanket reading and repeated rules with task-specific routing | Read applicable instructions and complete applicable contracts |
| Completion and handoff | Continue through the requested outcome; resume against live Git evidence | No speculative authority, unrelated work, or production edits in design-only work |
| ZIP intake | Inspect new/changed/unclassified/task-selected packages; reuse verified immutable bytes | SHA-256, member/manifest validation, full readiness reconciliation, frozen mirrors |
| Test selection | Reuse unchanged recorded results across edit/review/push | Existing workspace, Tier 2, lint, and structure triggers |
| Structural commands | Gate invocation also satisfies identical screening | All review thresholds, ownership dispositions, and blocking criteria |
| Prompt guidance | Add on-demand implementation, continuation, and design-only examples | No new mandatory startup checklist or model-specific permission assumption |

The root still requires the complete final typed authority, full affected
producer/consumer migration, deletion of obsolete unreleased paths, Sans-I/O
and dependency direction, contract version `1`, and documented unsafe boundaries.
Cargo job-count restrictions now have one owner in `crates/AGENTS.md`. Git-only
full-SHA evidence, existing-checkout/main-only work, user-change preservation,
and explicit permission for destructive operations or new branches/worktrees
remain. Connector edits explicitly use pinned remote evidence and non-forced
fast-forward publication, without claims about an unseen local working tree.

## Size measurement

UTF-8 byte counts and physical lines, not model token counts or speed benchmarks:

| File | Before bytes / lines | After bytes / lines |
| --- | ---: | ---: |
| Root `AGENTS.md` | 7,559 / 138 | 4,996 / 85 |
| `crates/AGENTS.md` | 7,702 / 145 | 4,609 / 78 |
| `docs/AGENTS.md` | 2,247 / 51 | 1,808 / 33 |
| `docs/implementation/AGENTS.md` | 1,866 / 31 | 1,753 / 30 |
| `docs/reviews/AGENTS.md` | 4,461 / 77 | 2,580 / 46 |
| Total | 23,835 / 442 | 15,746 / 272 |

The five instruction files are 33.9% smaller by bytes. These files are scoped,
not all loaded on every task. Some on-demand guides grew to make integrity,
evidence reuse, and permission conditions explicit; total repository text size
is not the optimization target. No agent latency or correctness gain is claimed.

## Validation performed

- Reconstructed the nine edited existing documents from connector responses and
  verified each byte sequence against its original Git blob SHA before editing.
- Ran `python /mnt/data/arcweft-instruction-audit/validate.py` over the proposed
  Markdown: original blob identities, UTF-8/newline/whitespace checks, balanced
  language-labelled fences, and added/changed link targets. Existing unchanged
  index links were not represented as newly validated links.
- Ran `git diff --no-index --check` on the scratch before/after directories:
  no whitespace diagnostics. Its differences exit status was `1`, not a
  successful runtime-test exit. The initial scratch wrapper incorrectly expected
  zero; that expectation was corrected and the checks rerun. No source gate or
  validation script was added to the repository.
- Reviewed the proposed diff and the six scenarios below against retained
  constraints. This is a document-policy review, not execution of model evals.
- Confirmed in `just/verify.just` that `structure-audit-gate` invokes the same
  scanner with `--fail-on-blocking`; no executable recipe changed.

| Review scenario | Expected routing and completion |
| --- | --- |
| Documentation typo | Applicable instructions and local context; no whole-doc stack, ZIP sweep, or Rust tests |
| Focused Rust fix | Relevant owner/contract and focused edit loop; existing mainline gates at completion |
| Cross-crate contract change | Complete affected consumer migration and all applicable workspace/structure/Tier 2 gates |
| ZIP intake | Verify selected/new/changed bytes and inherited requirements; unchanged bytes never imply unchanged readiness |
| Resume after compaction | Reconcile base, changed paths, contracts, and evidence; do not reconstruct an obsolete design from the summary |
| Design-only assignment | Complete design and requested artifact; no production patch, branch, or implementation overlay |

Not run: Rust compilation/tests, Clippy, runtime/Tier 2, and structural scanner.
This cut changes only instructions and documentation, with no Rust behavior,
schema, fixture, command recipe, archive, or generated artifact change. No
multi-model evaluation or performance benchmark was run. Scratch direct network
access was unavailable; repository evidence and publication use the connector.

This note records pre-publication review. Verify the resulting commit's complete
changed-file set and blob identities, its parent, and remote `main` after the
single non-forced update; report that delivery evidence separately. Do not infer
publication or CI success from this note alone.
