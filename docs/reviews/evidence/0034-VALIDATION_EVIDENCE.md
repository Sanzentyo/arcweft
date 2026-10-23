# Package validation evidence

Performed against the produced archive:

- maintained request byte identity: `0c570da664999507d1895813d65a707fb13726d48c489e8fd322c238a3361b78`;
- invalid intake note byte identity: `6f2d13f40738fb05c806f3283404afc8e1c9617d26aee0a42fad1c5b9f53e7f7`;
- retained parent rehash: `aa43429b6ffe5aac6489c94c7ff7a117ca1bbd43c764fed6ff4a1f3b5d540e06`;
- exact decision matrix cardinality: 15 numbered rows, 1 through 15;
- producer/consumer/deletion inventory rows: 226;
- test matrix rows: 671;
- every JSON and CSV sidecar parsed; every text member decoded as UTF-8;
- inventory/test IDs are unique;
- no prohibited prior authority owner appears outside the preserved invalid intake note;
- no uppercase generic status marker appears in normative package text;
- no `.rs`, patch, diff, overlay, production source, symlink, unsafe path, path traversal, drive path, or case-colliding member;
- compressed data integrity passes;
- fresh extraction reproduces every `MANIFEST.sha256` digest;
- deterministic rebuild with sorted members, timestamp `1980-01-01T00:00:00`, Unix mode `0644`, and DEFLATE level 9 is byte-identical.

Not performed: Cargo build/test, rustfmt, Clippy, structure audit, Tier 2, runtime codec execution, or production compile-fail checks. Those require the future implementation checkout; the local clone attempt was blocked by container network connectivity. Their exact required gates are specified in `ACCEPTANCE_COMMANDS.md` and the test matrix.
