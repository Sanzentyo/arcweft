# AGENTS.md — Arcweft Engine

Arcweft is a layered, verified, agent-native narrative engine written in Rust.
Arcweft source files use `.arcw`.

## Default: carry the requested work to completion

An implementation or fix request, including continuation of that work, authorizes
investigation, in-scope design, source/Cargo/test/fixture/documentation changes,
local validation, and change-caused repairs needed for the complete result.
Proceed without seeking approval for each step, owner migration, or technical
choice. A first patch, a design decision, a commit, or a context compaction is
not a reason to hand unfinished work back. Stop at the requested outcome, not
at an arbitrary small cut, and do not expand into unrelated improvements.

Use the user's current explicit instructions and established task intent over
repository workflow defaults and skill guidance. This does not override
higher-priority instructions or tool permissions. Read the applicable scoped
instructions; choose other context by the actual owner and acceptance criteria.
Current source is implementation evidence, not proof that every existing design
is correct. Maintained specifications and accepted contracts define the target;
resolve routine gaps and demonstrably stale guidance from evidence. For an
in-scope redesign, update the contract and all affected consumers together.

Missing implementation detail, several valid designs, file count, or a broad
migration is not an external blocker. Make the technical decision and record its
material rationale. Do not create a follow-up design request instead of finishing
resolvable work. Analysis-only and design-only requests retain their output scope.

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

## Real boundaries, not routine approval gates

Ask only when a material decision cannot be resolved from available evidence
and needs the user's intent, unavailable external facts, or an authorization
not already granted. Respect explicit user restrictions. Preserve user changes;
overlapping edits need intervention only when they cannot be reconciled without
losing or choosing between the user's intentions. Dirty state alone is not a
blocker. Do not invent approval flows for hypothetical risk.

When one operation is blocked, continue independent in-scope work. Name the
specific missing fact/permission or conflicting requirement, its source, and the
affected action; do not label a judgment call as a mandatory rule. An unavailable
test environment limits validation claims, not all investigation or delivery.
Never hide a failed required check or call unverified behavior complete.

## Git and delivery

Git is the sole VCS authority; record full commit SHAs. Inspect current `main`
and work on it in the existing checkout. Do not create another branch, worktree,
or checkout, or switch away from `main`, unless the user requests that operation.
This is an explicit workflow choice, not an inherent danger of those operations.
Do not discard/move unrelated WIP or use destructive Git operations without an
explicit request for the exact operation and verified targets.

At a coherent cut, inspect explicit staged paths/hunks, complete the applicable
validation, and commit/push without another approval round unless the user
requested a local hold. A commit boundary is not a request for permission to
continue the same goal. Do not publish speculative WIP or mix unrelated goals.
For requested connector edits, inspect a pinned remote base and the complete
diff, publish with a non-forced fast-forward, and reconcile a moving head rather
than overwrite it. Do not claim to know an unobserved local working tree.

Report the delivered result and actual evidence, with failures, unavailable
checks, remaining work, and material deviations distinguished. Keep durable task
state in `docs/implementation/`, not in instructions or stable design chapters.

## References by task

- Rust/Cargo, including tools and API docs: [crates/AGENTS.md](crates/AGENTS.md).
- Documentation: [docs/AGENTS.md](docs/AGENTS.md); [index](docs/README.md).
- Evidence/handoffs: [implementation instructions](docs/implementation/AGENTS.md).
- Requests/ZIPs: [review instructions](docs/reviews/AGENTS.md).
- Instruction maintenance: [agent workflow](docs/implementation/agent-workflow.md).
