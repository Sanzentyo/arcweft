# Lang-01.5.1.3 — generated artifact runtime-binding fail-closed contract

## Sequence position

This is the third correction split from Lang-01.5.1. It consumes the accepted
external-module metadata and Activity binding facts produced by the
single-manifest topology. It does not add artifact loading or execution.

## Why this split is required

Lang-01.5.1 explicitly excludes Rust/WASM/process artifact execution while its
test matrix E-19 requires a runtime attempt to fail with
`runtime-binding-missing` unless the host supplies an exact binding for the
metadata-pinned artifact.

The repository currently retains the complete accepted `AdapterArtifact`,
target, ABI, export, and Activity facts, but has no shared runtime artifact
binding catalog or exact lookup key. Reusing an ordinary callable name,
Activity ID, file path, or adapter profile alone would lose accepted metadata
identity and could bind the wrong artifact after an overlay or rebuild.

## Decisions required

1. Define the exact host binding key. It must say which accepted facts
   participate, including at least:
   - external-module import identity and mount;
   - target family and ABI;
   - package/module/version;
   - metadata ABI hash;
   - artifact path, raw digest, and size;
   - export identity and, for Activity exports, abstract `ActivityId` plus
     interface/state hashes.
2. Define the Sans-I/O runtime binding catalog and registration API. Hosts
   provide already constructed typed bindings; the core/runtime must not read
   paths, load libraries, spawn processes, or parse metadata.
3. Define where the accepted topology projects the exact key into the runtime
   plan or launch product without re-decoding metadata.
4. Define the structured missing/mismatch errors and the point at which each is
   raised. Missing, stale, wrong-family, wrong-ABI, wrong-artifact, and
   wrong-export bindings must not fall back by name.
5. Define lifetime and revision correlation with accepted topology/LSP
   generations so a binding from an older metadata revision cannot be reused.

## Required implementation order

1. Specify the immutable exact binding key and error types.
2. Add the host-owned Sans-I/O binding catalog.
3. Project accepted metadata/export facts into the runtime launch product.
4. Validate exact correlation before an external call or Activity start.
5. Add fail-closed tests before adding any successful host test binding.
6. Add one exact in-memory successful binding test without implementing
   filesystem/provider loading.

## Tests the contract must specify

- selected generated export with no host binding returns
  `runtime-binding-missing`;
- wrong import, mount, package, module, version, target family, target ABI,
  metadata ABI hash, artifact path/hash/size, or export identity is rejected;
- Activity identity/interface/state hash mismatch is rejected;
- a binding accepted for a prior topology revision is stale after a metadata
  overlay changes;
- an unselected generated module cannot be invoked through a host binding;
- one exact in-memory host binding is selected deterministically;
- no fallback occurs through callable spelling, Activity spelling, basename,
  adapter profile, or filesystem path;
- runtime-plan/launch serialization, if it carries the key, round-trips every
  field without a compatibility reader;
- missing/mismatch failures do not partially start an Activity or enqueue host
  work.

## Constraints

- Do not redesign accepted metadata decoding, hash validation, mount
  projection, or Activity export reconciliation without a concrete flaw.
- Do not implement dynamic library loading, WASM instantiation, process spawn,
  provider discovery, Cargo, WIT parsing, or artifact download.
- Keep `arcweft-core` Sans I/O.
- Do not reduce the key to strings when typed identities/digests already exist.
- Do not add fallback adapters, aliases, version migration, or last-known-good
  binding acceptance.
- Test typed behavior, not implementation source spellings.

## Expected output

Return an implementation-ready final contract with:

- final Rust shapes and owning crates;
- exact binding-key canonicalization;
- runtime/host dependency direction;
- topology-to-runtime projection;
- error taxonomy and revision rules;
- implementation/deletion order;
- complete positive, negative, stale, and round-trip test matrix;
- explicit non-goals; and
- `OPEN_QUESTIONS=0`.
