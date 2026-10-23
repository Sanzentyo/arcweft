# Package material verification declaration

| Material | Verification actually performed |
|---|---|
| `REQUEST_SPEC.md` | Copied from the uploaded request; SHA-256 recorded in `PACKAGE_STATE.json`. It is the sole normative specification. |
| `FINAL_CONTRACT.md` | Repository-aware analysis validator result: `FALLBACK`. Required machine markers normalized and checked. |
| Repository source | Checkout at `UNAVAILABLE`; equality with observed `origin/main`: `not verified`. |
| `AGENTS.md` evidence | Every discovered `AGENTS.md` copied and SHA-256 indexed; count: `0`. |
| Existing Rust checks | Exact commands, exit codes, duration, and logs are in `VERIFICATION_STATUS.md` and `verification/`. |
| Source modification | Final worktree state: `not clean/unavailable`. No implementation was intentionally performed. |
| Proposed tests and implementation order | Contractual/planned only; not executed as implementation because the user expressly prohibited implementation. |

No artifact in this ZIP should be interpreted as having been verified beyond the explicit boundary above.
