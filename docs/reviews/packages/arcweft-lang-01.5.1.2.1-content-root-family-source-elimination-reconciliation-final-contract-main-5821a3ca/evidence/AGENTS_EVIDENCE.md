# AGENTS evidence

- Repository: `Sanzentyo/arcweft`
- Revision: `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`
- Path: `AGENTS.md`
- Git blob: `e91f99213dde67953beda6aa078c370a8dc4541d`
- Read scope: complete file, through the final line

Rules materially applied:

- inspect the latest repository philosophy/structure before contract work;
- preserve verified substrate unless a concrete defect requires change;
- extend Arcweft-owned enums/owners directly rather than adding an extension
  trait, ad hoc helper, or duplicate lookup table;
- no source-gate automation; deletion is proven by typed behavior;
- no compatibility aliases/readers or fallback acceptance;
- compare returned review ZIPs against intake records and hashes;
- record actual validation scope and do not infer implementation readiness from
  a filename;
- run focused/workspace/Tier 2/structural validation for production completion;
- keep core/data-format ownership Sans I/O and respect crate layering.
