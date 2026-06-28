# Release Trust Fixtures

This directory contains static policy metadata for Seq-02.9. The Rust integration
tests generate AWFB/AWFR byte fixtures in test-owned tempdirs so that signatures,
content roots, patch artifacts, and cache paths remain deterministic but do not
need to be committed as large binary blobs.

## Test-only key policy

The key under `keys/` is deliberately deterministic and is suitable only for
local tests. It must never be used for production releases or copied into a
production signing profile.

## Fixture matrix

See `matrix.json` for the full list of success and failure cases and the stable
machine-readable evidence code expected from `arcw release verify --json`.
