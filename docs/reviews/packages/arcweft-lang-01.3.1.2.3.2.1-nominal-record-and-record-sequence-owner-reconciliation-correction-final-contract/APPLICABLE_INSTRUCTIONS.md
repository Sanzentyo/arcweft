# Applicable instruction digest

The package follows these repository principles:

- one final typed authority; no copied side tables, fallback readers, or
  parallel models;
- deletion-driven migration: typed producer/consumer migration and immediate
  removal of superseded surfaces;
- preserve the syntax -> HIR -> sema -> runtime-plan/core dependency direction;
- add behavior to Arcweft-owned enums/owners through inherent implementations
  rather than extension traits or ad-hoc helpers;
- treat source scans as discovery, never as acceptance gates;
- deliberate public visibility and private carrier fields;
- deterministic error precedence and typed errors;
- no production edit in a design-only package;
- `READY_FOR_IMPLEMENTATION` only with all result-changing decisions closed;
- Rust formatting, check, Clippy `-D warnings`, tests, and structural audit at
  implementation gates; and
- exact verification claims, including explicit non-run gates.

The supplied Rust Skill was read in full. Its practical constraints applied
here include newtype/typed boundary preference, careful public API design,
iterator-based transformations where clear, no casual allow/unsafe/macro
expansion, and Serde only where the enclosing architecture currently requires
it.
