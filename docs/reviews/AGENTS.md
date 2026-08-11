# Review request and package instructions

Read `docs/reviews/README.md` before handling a request or returned archive.
These rules apply to `requests/`, `packages/`, `designs/`, and ZIPs dropped into
the review inbox.

## Repository evidence

- Git is the only version-control authority. Record the full inspected Git
  commit SHA and dirty/clean state; do not request or record a Jujutsu change
  ID. This Git-only rule supersedes older request text that asks for matching
  Git/Jujutsu identities.
- Read the complete current request, its primary and parent requests, the intake
  note, every selected predecessor, and every maintained production consumer
  named by them. A filename, sequence label, or `FINAL` suffix is not evidence.
- Current production and maintained documentation may reveal that an old
  package expression is superseded. Do not force production back to it without
  a concrete flaw in the current authority.

## ZIP intake

- Treat a directly attached ZIP or an unclassified ZIP under `docs/reviews/`
  as intake. Enumerate review ZIPs again when resuming package work and at each
  reviewable push cut.
- Verify SHA-256, byte length, member set, internal manifest, member hashes,
  request copies, `FINAL_STATUS`, `OPEN_QUESTIONS`, schemas, matrices,
  traceability, and repository evidence before adjudicating readiness.
- Keep every sidecar inside the returned ZIP. Do not require adjacent summary,
  status, hash, or manifest files.
- Keep throwable Markdown requests in `requests/`, retained source archives in
  `packages/zips/`, their searchable extracted contents in
  `packages/<zip-basename>/`, and reusable accepted material in a
  sequence-named `designs/` directory. Within each design directory, retain
  source archives in `zips/` and place their searchable extracted contents in
  that same design directory after safe collision checking.
- Treat extracted package and design files as frozen mirrors of their retained
  ZIP members. Historical request copies, ledgers, and other returned evidence
  may therefore preserve paths that were current when the ZIP was produced;
  update maintained navigation instead of rewriting those mirrored files.
- Record an external archive's verified path and hash in its implementation
  intake note when copying the binary into Git is not useful.

## Readiness and design-only work

- A design-only assignment must not edit production code, tests, fixtures,
  manifests, branches, patches, PRs, or implementation overlays.
- Use `READY_FOR_IMPLEMENTATION` only when every result-changing decision is
  closed and `OPEN_QUESTIONS.md` is exactly `none` as required by the request.
- Do not return `NOT_READY` merely because repository investigation is large.
  Continue the same assignment and close every decision current Git evidence
  can resolve. Reserve `NOT_READY` for a genuinely external unresolved
  authority and identify it exactly.
- A broad sequence request is not implementation-ready design by itself. Split
  underdesigned topics into follow-up requests and keep them out of the active
  implementation acceptance criteria.
- Do not introduce compatibility aliases, wrappers, dual readers, migration
  maps, source-string reparsing, source gates, removed-syntax diagnostics, or
  implementation overlays for unreleased contracts.

## Follow-up requests

- Do not give the user a throwable request name until its independently usable
  Markdown file exists in `requests/`. Always provide the repository path.
- Preserve the parent sequence when assigning a child number. Include the split
  reason, exact decisions required, precedence, non-goals, consumer inventory,
  implementation order, required tests, constraints, and exact output archive.
- Group topics only when they must be designed together to avoid incompatible
  contracts. Otherwise keep requests small and sequential.
- State that accepted and validated substrate must not be redesigned without a
  concrete repository-evidenced flaw.
- Link every blocker request from the relevant implementation intake or status
  note.
