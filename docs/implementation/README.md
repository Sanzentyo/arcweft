# Implementation Status

This directory records the current implementation state of Arcweft Engine.

Design specifications remain in the numbered `docs/` chapters. Files here describe what exists in the Rust workspace today, what has been verified, and what is intentionally deferred.

## Current Milestone

Phase 0 / Phase 1 minimal Rust workspace:

- Cargo workspace skeleton.
- Foundational ID, source anchor, Need, and dialogue surface model crates.
- Stub language syntax and CLI crates.
- No renderer, Servo, audio, camera, USB, MCP, or Cranelift JIT implementation.

## Files

- `phase-0-1-workspace.md`: current crate layout, public types, verification status, and deferred work.
