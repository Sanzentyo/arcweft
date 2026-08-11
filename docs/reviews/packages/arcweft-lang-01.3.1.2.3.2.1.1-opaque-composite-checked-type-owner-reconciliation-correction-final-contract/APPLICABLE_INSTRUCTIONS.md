# Applicable instruction inventory

The following instruction scopes were applied:

1. Project instruction to read the Rust Skill completely.
2. Project instruction to inspect the latest/pinned `AGENTS.md` before design.
3. Root Arcweft principles: one typed authority, deletion-driven migration,
   no source/display reparsing, no fallback/parallel model, and inherent
   behavior on Arcweft-owned enums.
4. Design-only package rule: no production code, Cargo manifest, patch, diff,
   overlay, branch, PR, compatibility layer, or fabricated execution log.
5. Request-specific constraints and retained parent substrate.

The package extends original owners (`RuntimeCheckedType`, `RuntimeValue`,
`AcceptedNominalSemantics`, `RuntimeTypeShape`, `RuntimeResolvedVariant`, AWBC
types, CharacterDialogue types) directly. It intentionally avoids helper
traits, global producer registries, side tables, or hard-coded VM spelling
switches.
