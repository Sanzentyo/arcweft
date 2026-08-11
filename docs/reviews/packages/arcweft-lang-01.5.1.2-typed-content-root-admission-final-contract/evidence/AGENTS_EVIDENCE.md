# AGENTS.md evidence

- Repository: `Sanzentyo/arcweft`
- Pinned revision: `23ed5d93824630d8ead9092d32f7fc70f0a8f314`
- Blob SHA: `ea4a46132ff8cd004f860c89c854e4cbfe807d86`
- Read boundary: complete file, lines 1–447.

Applied repository rules:

- use the Rust skill before Rust/Cargo/test design;
- preserve crate layering and Sans-I/O data models;
- put missing behavior on the owning Arcweft enum/boundary type;
- replace unreleased provisional shapes directly; no deprecated fields, dual readers, aliases, or shims;
- root-cause edits and compile-fail/public behavior evidence instead of source gates;
- use typed conversion/context boundaries rather than endpoint-named helpers;
- run Tier 2 for a multi-crate public contract affecting runtime/bundle/LSP paths;
- perform structural audit for public contract/dependency changes;
- package-driven completion must satisfy the full request, with verification limits stated in the ZIP.
