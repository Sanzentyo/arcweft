# AGENTS.md — Arcweft Engine

## Start here

Arcweft is a layered, verified, agent-native narrative engine written in Rust.
Arcweft source files use the `.arcw` extension.

- Work from the latest accepted `main` and inspect the current Git state before
  changing files.
- Read `docs/README.md` and every more-specific `AGENTS.md` that applies to the
  paths being changed. Deeper instructions add to or override this file within
  their directory scope.
- Before changing Rust, Cargo manifests, build scripts, Rust tests, benches,
  Rust tools, or Rust-facing documentation, read every applicable Rust skill
  completely and follow it.
- Treat current source, maintained stable documentation, and accepted design
  contracts as evidence. Conversation summaries and filenames are not
  implementation authority.

## Repository-wide invariants

- Preserve the documented layer direction. Lower layers must not depend on
  higher layers, and Sans-I/O crates must remain Sans I/O.
- Prefer one final typed authority. Do not add parallel models, dual readers,
  fallback resolvers, source-string reconstruction, or copied side tables.
- Replace unreleased internal contracts directly with the selected final
  model. Compatibility requires evidence of a released artifact, persisted
  user data, an external consumer, or an explicit user requirement.
- Use deletion-driven migration: remove obsolete types, variants, functions,
  helpers, and success branches once their final replacement is available, then
  fix every exposed consumer. Do not repair an old path that is scheduled for
  deletion.
- When an Arcweft-owned enum or boundary type lacks domain behavior, add that
  behavior to the owning type or its legitimate context when dependency
  direction permits. Avoid scattered match helpers, extension traits, and
  stringly wrappers.
- Represent language and runtime rules generically through typed grammar,
  registries, schemas, and identities. Do not hard-code one spelling or one
  nominal type when the rule is general.
- Validate through typed APIs, executable behavior, codecs, compile checks,
  lints, deterministic generated artifacts, and structured dependency graphs.
  Source spelling and file placement are review aids, not acceptance evidence.
- Prefer deterministic runtime and build behavior.
- Do not use `unsafe` unless it is isolated behind a clearly named boundary
  with a documented invariant.
- Preserve user changes. Do not reset, discard, overwrite, or broadly move a
  dirty working tree to make an unrelated task easier.

## Git-only workflow

Git is the sole version-control authority for this repository.

- Use `git status`, `git diff`, `git log`, `git show`, and related Git commands.
  Do not use Jujutsu, record Jujutsu change IDs, or require matching Git and
  Jujutsu identities. Repository evidence uses the full Git commit SHA.
- Keep a small, fully isolated change directly on `main` when that is safe.
  Stage explicit paths or hunks and inspect `git diff --cached` before commit.
- For independent, parallel, risky, or long-running work, or when the primary
  checkout contains protected WIP, use a separate Git worktree and a short-lived
  task branch named `codex/<topic>`.
- Keep task branches local by default. After validation, update them onto the
  latest `main`, integrate with a fast-forward when practical, push `main`, and
  remove the temporary worktree and branch when safe.
- Do not push speculative WIP refs. Do not mix unrelated goals in one commit or
  carry an independently completed policy, request, or implementation cut into
  the next goal.
- At a reviewable cut point, validate, commit, and push autonomously unless the
  user explicitly asks to hold changes locally. A cut is a coherent result, not
  every edit and not a bag of unrelated work.
- Never use destructive Git operations such as `git reset --hard` or forced
  checkout to dispose of changes unless the user explicitly requests that exact
  operation and the targets have been verified.

## Scope and completion

- For package- or brief-driven work, derive acceptance criteria from the full
  source package and compare them with current implementation evidence. Do not
  redefine completion around the easiest implemented subset.
- If a required boundary remains underdesigned, do not guess it. Record the
  exact non-goal or blocker in `docs/implementation/` and create an independently
  throwable request under `docs/reviews/requests/` when design work is needed.
- Keep transient progress and task-specific checklists out of this file and out
  of stable design chapters.
- Do not hide failures. Distinguish passed, failed, blocked, and not-run
  validation.

## Scoped instructions

- Rust, Cargo, tests, benches, and workspace structure: `crates/AGENTS.md`.
  Treat it as the workspace Rust policy even for applicable Rust files outside
  `crates/`.
- Documentation authority and stable design: `docs/AGENTS.md`.
- Current implementation evidence and validation records:
  `docs/implementation/AGENTS.md`.
- Review requests, returned ZIPs, and package intake:
  `docs/reviews/AGENTS.md`.

## Final report

Report only what the current task established:

- changed files or inspected scope;
- validation actually run and its result;
- failures, blocked validation, and intentionally skipped tiers;
- remaining work and explicit non-goals; and
- design deviations, if any.
