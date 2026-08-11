# Explicit non-goals

This contract intentionally does not design or implement:

- dynamic-library loading or symbol resolution;
- Rust artifact execution or ABI invocation;
- WASM engine selection, component instantiation, or WIT parsing;
- process spawn, transport I/O, framing, lifecycle, or sandbox policy;
- provider discovery, plugin directories, environment-variable lookup, Cargo metadata/builds, downloads, or artifact installation;
- filesystem existence, permission, timestamp, canonicalization, or basename fallback;
- metadata re-decoding, alternate metadata readers, hash validation redesign, mount projection redesign, or Activity export reconciliation redesign;
- successful generated artifact execution semantics, arguments/results ABI, traps, retries, cancellation, or resource accounting beyond the pre-host selection gate;
- persistence or serialization of live bindings/catalogs;
- a global/stable binding ID across products, profiles, revisions, processes, or LSP generations;
- a new aggregate key digest authority;
- publication of a broader project-topology revision not already accepted by the repository;
- compatibility aliases, wrappers, dual readers, schema migration, version coercion, source gates, or last-known-good binding acceptance;
- lookup by callable spelling, Activity spelling/ID alone, mount, basename, adapter profile, adapter ID, path, package/module name, or digest alone;
- redesign of `ActivityHostRegistry` as an artifact provider registry;
- production code, tests, branches, patches, PRs, or implementation overlays in this returned design archive.

A future artifact-execution contract may define concrete `F`/`A` binding types and invocation semantics. It must consume the exact selected binding from this contract rather than bypassing or replacing the key/product/catalog authority.
