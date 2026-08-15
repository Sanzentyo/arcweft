# Verification scope

## Statically verified in this return

- exact current request bytes and SHA-256;
- exact previous INVALID bytes and SHA-256;
- latest GitHub `main` head and selected full SHA;
- current commit-pinned source symbols/line evidence listed in the evidence CSV;
- current workspace package names;
- package naming, one top-level directory, safe member paths, no case collision,
  no symlink, no production source/patch/overlay;
- internal manifest and ZIP extraction hash parity;
- `OPEN_QUESTIONS=0` and version-1-only design text;
- no placeholder comment tokens such as `/* ... */` in normative API files;
- no validation command names a nonexistent AOT/VM package.

## Design decisions not executable until implementation

Every Rust API, codec change, compile-clean phase, unit/integration/UI test,
Clippy result, and runtime VM/JIT/AOT/restore behavior is a normative
implementation requirement. It was not executed against production because
this is a design-only package and no checkout/patch is present.

## No overclaim

The archive's `READY_FOR_IMPLEMENTATION` status means the design has no open
questions and supplies exact owners/APIs/order/grammar/tests. It does not mean
production implementation or Cargo validation has already passed.
