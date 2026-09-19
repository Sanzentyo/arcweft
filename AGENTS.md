# AGENTS.md — Arcweft Engine

Arcweft is a layered, verified, agent-native narrative engine written in Rust.
Arcweft source files use `.arcw`.

## Task context

Use current source, maintained specifications, and accepted contracts as
evidence; a conversation summary or filename is not implementation authority.
Read the scoped `AGENTS.md` files for paths being changed. Load other documents
and task-matching skills only as needed; follow their routers to relevant
references rather than reading every document mentioning the language.

- Rust/Cargo, including Rust tools and API documentation outside `crates/`:
  [crates/AGENTS.md](crates/AGENTS.md).
- Documentation: [docs/AGENTS.md](docs/AGENTS.md).
- Implementation state: [docs/implementation/AGENTS.md](docs/implementation/AGENTS.md).
- Requests or returned packages: [docs/reviews/AGENTS.md](docs/reviews/AGENTS.md).
- Document discovery: [docs/README.md](docs/README.md), an index, not a reading list.
- Prompt/instruction maintenance or a difficult handoff:
  [agent-workflow.md](docs/implementation/agent-workflow.md).

## Design invariants

- Preserve layer direction and Sans-I/O boundaries. Select the complete domain
  model from ownership, invariants, and all affected consumer needs, not minimum
  patch size, type count, migration cost, or short-term speed.
- Keep one final typed authority. Put behavior on its owning type or legitimate
  context; express general rules through typed grammar, schemas, registries,
  identities, or state machines. Repair the general boundary and migrate all
  affected producers/consumers, rather than adding example-specific exceptions,
  parallel models, copied side tables, fallback resolvers, or source reconstruction.
- Replace unreleased internal contracts directly and delete obsolete paths when
  their final replacement is available. Do not repair a path slated for deletion
  or retain dual readers. Compatibility needs a released artifact, persisted user
  data, a known external consumer, or an explicit user requirement.
- Keep every Arcweft-owned contract version marker at `1`, including schemas,
  codecs, wire/protocol/ABI, saves, snapshots, caches, digest domains, and generated
  source. Evolve shapes in place; no `V2`/`V3` types/domains or legacy readers.
  Reconcile a touched non-`1` marker to `1`. A compatibility exception needs
  explicit user direction backed by released artifacts, user data, or an external
  consumer.
- Prefer deterministic runtime/build behavior. Isolate any `unsafe` behind a
  named boundary with a documented invariant.
- Establish behavior through typed APIs, executable tests, codecs, compile
  checks, lints, deterministic artifacts, and structured dependency graphs.
  Source spelling and file placement are review aids, not acceptance gates.

## Work and completion

Derive completion from the requested outcome and full applicable contract, not
from the first compiling subset. Complete the coherent change, affected consumer
migration, selected validation, and change-caused fixes without stopping after a
first implementation. Do not freeze temporary scaffolding or split one semantic
authority merely to make a smaller cut. Stop when the requested outcome is met;
do not add unrelated improvements.

Resolve in-scope design choices from repository evidence. Do not invent an
external contract or silently override an accepted one. When missing external
information, conflicting authorities, permissions, or overlapping user edits
block a decision, isolate that blocker and continue independent in-scope work.
Record the exact unresolved decision and, when needed, a usable design request.
Analysis-only and design-only tasks do not authorize production edits.

## Git and permissions

Git is the sole VCS authority; record full commit SHAs, not Jujutsu identities.
Inspect current `main` and preserve user changes. Use the existing checkout;
no new branch, worktree, checkout, or switch away from `main` without an explicit
request for that operation. Dirty state is not permission to discard or move WIP.
If the requested edit cannot preserve it, stop that edit and seek direction.

Stage explicit paths/hunks and inspect the staged diff. At a validated, coherent
cut, commit and push unless the user requested a local hold. Do not combine
unrelated goals or publish speculative WIP. Destructive Git operations require
an explicit request for the exact operation and verified targets.

For explicitly requested GitHub-connector edits, use remote `main`, inspect a
pinned base and the complete proposed diff, and publish one coherent commit with
a non-forced fast-forward. Recheck a moving head and reconcile changes rather
than overwriting them. Do not claim to know an unobserved local working tree.

Report the result, validation actually performed, failed/blocked/not-run checks,
remaining work, non-goals, and design deviations. Keep task state in
`docs/implementation/`, not in instructions or stable specification chapters.
