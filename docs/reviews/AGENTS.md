# Review requests and packages

For ZIP intake, integrity checks, extraction, and storage, use
[README.md](README.md). A request-only wording edit does not trigger archive
intake. Root Git/permission rules and documentation authority still apply.

## Scope and evidence

Read the complete active request and applicable contract/package, plus its
current intake decision. Follow primary/parent/predecessor references that supply
inherited acceptance criteria, precedence, or a decision affecting this task;
include those requirements in the acceptance map. Inspect all affected production
consumers, not every unrelated consumer ever named in the sequence. If an
inheritance boundary is unclear, resolve it rather than silently dropping scope.

Use current repository evidence; a filename, sequence number, or `FINAL` suffix
is not readiness evidence. Do not restore a superseded package expression or
redesign accepted, validated substrate without a concrete repository-evidenced
flaw. Use full Git SHAs, not Jujutsu identities, even if an old request asks for
both. Distinguish observed local state from connector-only remote evidence.

## Completion boundaries

Design-only work must not edit production code, tests, fixtures, manifests,
branches, patches, PRs, or implementation overlays. Complete the requested design
and artifact; implementation requires an implementation assignment.

For readiness adjudication, close every result-changing decision and satisfy the
request's complete evidence contract. Use `READY_FOR_IMPLEMENTATION` only with
those decisions closed and `OPEN_QUESTIONS.md` exactly `none` when required.
Large investigation is not a `NOT_READY` reason: resolve what current evidence
can decide, and name genuinely external unresolved authority precisely.

Do not split mutually constraining decisions into an easy subset or introduce
compatibility wrappers, dual readers, reparsing, source gates, or removed-syntax
diagnostics for unreleased contracts. Preserve archive/extracted mirror bytes;
edit maintained navigation and intake decisions, not returned historical evidence.

## Follow-up requests

Create an independently usable Markdown file in `requests/` before presenting
its name; provide its repository path and link it from the relevant intake or
state note. Preserve the parent sequence and include the split reason, exact
decisions, precedence, non-goals, affected consumers, implementation order, tests,
constraints, and output archive. Split only semantically independent topics;
request size or implementation convenience is not sufficient.
