# seq04.8.1 persistent-cache bytecode/link fixture

This fixture directory records the product-byte equivalence boundary for the seq04.8.1 overlay package.

- `bytecode-rebuilt.awbc.hex` and `bytecode-reused.awbc.hex` have identical bytes.
- `product-bytes-rebuilt.awfb.hex` and `product-bytes-reused.awfb.hex` have identical bytes.
- `equivalence-manifest.json` records the shared SHA-256 digests.

The `.hex` files are deterministic package fixtures. After applying the overlay to a full Arcweft checkout, focused tests should produce full AWBC/AWFB product bytes from source rebuild and cache reuse paths and assert exact byte equality.
