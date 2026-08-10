# Agent call lexical owner closure

Date: 2026-08-08

Inspected Git commit: `ab9c7942ff1a280a808c6b72e49424416e30608f`

Working-tree state at inspection: dirty on `main` with only this coherent
semantic-owner cut. One checkout and the normal shared Cargo target were used;
all Cargo validation used four build jobs.

## Performed

Every final call fact now retains the exact ordinary Function declaration that
lexically owns its expression. The producer reads the already-staged checked
callable body scope and `CheckedCallableDeclaration`; it does not scan source,
derive an owner from a name, or publish another callable/body index.

Lexical containment deliberately includes closures. A call inside a closure
owned by an ordinary function therefore retains that ordinary function as its
entry-policy owner, while independent checked callable bodies remain distinct.
The same producer is used by selected calls, recovered/ambiguous calls,
associated-call recovery, and dialogue/content call facts.

This record supersedes the Agent-owner blocker classification in
`2026-08-08-character-dialogue-semantic-runtime-public-switch.md`.
Lang-01.1.1 ordinary-function role and Lang-01.1.1.2 project nominal
resolution (including the retained corrections) are already returned and
implemented. The observed `enclosing_callable = None` result was a producer
regression, not an external-design dependency, and those requests must not be
dispatched again.

## Exposed next boundary

After the lexical owner was present, an exact selected Agent controller reached
runtime semantic projection and failed because
`AgentIntrinsicSignatureId::Observe` has no projection in
`RuntimeResolvedCallTarget`. That is the next implementation boundary. It is
not repaired in this cut by restoring the deleted HIR/source-name Agent reader,
mapping Agent operations to pure Core intrinsics, or emitting the old stringly
`LineEffectRequest::Call` form.

## Validation

- `cargo test -p arcweft-lang-sema --lib agent_intrinsic --all-features
  --jobs 4`: 2 passed;
- `cargo test -p arcweft-lang-sema --lib --all-features --jobs 4`: 185 passed;
- `cargo check --workspace --all-targets --all-features --jobs 4`: passed;
- `cargo clippy --workspace --all-targets --all-features --jobs 4 --
  -D warnings`: passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed; and
- `just structure-audit` and `just structure-audit-gate`: passed at 2,126
  files, 2,003 Rust files, 990,283 Rust LOC, 94 workspace packages, 182 review
  triggers, and zero blocking findings.

No Tier 2 runtime target is selected for this semantic-only cut. The next
Agent runtime projection cut must select its compiler, AWBC, runner, CLI, and
save/replay coverage together.

## Non-goals

- no compatibility alias, dual reader, source reconstruction, or removed
  syntax diagnostic;
- no Agent runtime operation ABI or temporary registered-callable mapping;
- no CSS/Takumi path; and
- no new review request for already returned Lang-01.1.1.2 work.
