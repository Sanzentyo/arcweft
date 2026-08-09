# Proof grouped-use member budget decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

No historical focused-test result is inherited by this recovered decision.

Grouped-use members use the existing inclusive
`SyntaxLimit::DeclarationMembers` budget, whose maximum is 1,024. Every
authored member, including a recovered member, is charged exactly once before
its events are accepted. Exact limit commits; one over rejects the whole syntax
transaction without changing the accepted generation or allocator state.

This keeps `SyntaxRole::Element(u32)`, alias `Field(u16)`, and recovery
`Recovery(u32)` identities unique without widening a role or introducing a new
limit. Current acceptance must exercise exact/one-over parser and database
behavior; source-text scans are not accounting evidence.
