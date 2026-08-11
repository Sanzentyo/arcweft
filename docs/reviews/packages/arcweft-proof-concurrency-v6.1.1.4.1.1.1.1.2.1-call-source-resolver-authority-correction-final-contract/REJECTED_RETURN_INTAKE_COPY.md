# Rejected-return intake copy

Repository path:
`docs/implementation/2026-07-29-proof-01-1-1-4-1-1-1-1-2-call-recovery-return-intake.md`

Repository blob SHA:
`e4b35455d95d0a12677127e8a797a4771d19a291`

Status: `RETURNED_REJECTED_NOT_READY_FOR_IMPLEMENTATION`

Rejected archive:
`arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2-call-recovered-argument-schema-correction-final-contract.zip`

SHA-256:
`BC8DE35E8C4D69008344EC44B9CFF1C5C59EE17ECB2CA54006B0ECF6EE923B50`

Mechanical validation succeeded, but repository intake rejected the package for
result-changing conflicts:

1. It did not inspect the required predecessor archives.
2. It introduced `HirCallSourceSurface`, its own component map/reader, a second
   `Whole`, and stored optional/inapplicable values instead of extending the sole
   `HirSourceIndex`/`HirSourceQuery` authority.
3. Its fixtures used `name: value` and prefix `...value`, not Arcweft
   `name = value` and postfix `value...`.
4. Its cursor rules made the opening token active, assigned commas backward, and
   made every close one-past, contradicting production behavior.
5. It collapsed missing callee/unresolved dot and omitted associated receiver,
   separator, arity, terminal nominal, and `HirAssociatedCallSyntax` states.
6. It discarded the qualified poisoned `TypeId` of present-invalid explicit
   type syntax.
7. It created duplicate argument-index/limit types and replaced the 256-candidate
   `CallableLimits` authority with an unrelated two-candidate resolver.
8. It replaced rather than integrated `resolve_call_target`,
   `CallTargetFacts`, checked slot facts, work counters, probing, replay, and
   signature projection.
9. It contradicted exactly-once argument checking and the zero-resolver
   associated-arity failure path.
10. It defined no complete central attached owner for argument/type geometry,
    so deleting `ArgumentListSyntax` lost information while retaining it created
    a dual reader.
11. It used undefined placeholder identities.
12. It claimed unreachable Call `RecoveryOperand` 1023/1024 tests despite the
    128-argument preflight.
13. It lacked a complete deletion-driven consumer matrix.

Useful direction retained by this replacement:
known Call + typed poison, authored-order argument form, no fabricated IDs,
root-owned recovery operands, retained dot value/nominal evidence, one ordered
explicit call type-application owner, and deletion-driven migration.
