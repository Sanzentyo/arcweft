# Seq-03 Remaining Package Dispatch

## Purpose

This file is the throw-ready index for the remaining seq03 work after the
production application of seq03.1 through seq03.6.

Seq03.1-03.6 are already production-applied: generation runtime table,
generation-bound task dispatch, code-generational `BundleSession` behavior,
`WindowedRuntimeOwner`, scene ownership integration, and the typed multi-entry
runtime start API. Do not ask follow-up agents to redesign that substrate unless
they find a concrete implementation flaw.

## Throw These Requests

### Next wave: design in parallel, apply sequentially

Send these requests at the same time if desired:

- `docs/reviews/requests/2026-06-28-seq-03.7-windowed-live-patch-smoke-fixtures-package.md`
- `docs/reviews/requests/2026-06-28-seq-03.8-windowed-live-patch-ingress-adapter-package.md`

Expected result from each agent: a zip package with `overlay/`, tests,
implementation note, validation log, non-goals, and direct apply instructions.

Apply order after zip return:

1. Apply seq03.7 first, because smoke fixtures should establish the concrete
   AWFB/patch regeneration and observation path.
2. Apply seq03.8 after seq03.7, because ingress should reuse the
   fixture/regeneration path as validation evidence.

## Suggested Parallel Prompt

Use this prompt when launching the first wave:

```text
You are working on Sanzentyo/arcweft main after seq03.1-03.6 have been
production-applied. Please implement the attached request as a zip overlay
package, not as a direct repository commit.

Read the request markdown fully and treat it as the acceptance criteria. Do not
redesign the already-applied generation runtime table, generation-bound host
task dispatch, code-generational BundleSession behavior, WindowedRuntimeOwner
scene integration, or typed multi-entry start API unless current source evidence
shows a concrete flaw.

Return a zip containing:
- README.md with assumptions, apply order, acceptance criteria, and non-goals
- overlay/ implementation files
- tests under the owning crate(s)
- docs/implementation/<seq-name>-2026-06-28.md
- VALIDATION.md with commands run and honest blockers
- patches/ only if direct overlays are insufficient

Keep runtime-driver Sans I/O. Keep filesystem/network/watch/release/trust
adapter work outside arcweft-runtime-driver. Do not use unsafe or unstable Rust.
Prefer typed Arcweft-owned APIs over stringly helpers or compatibility shims.
```

## Completion Boundary

Seq03 can be considered complete only after:

- `scene_windowed.rs` uses `WindowedRuntimeOwner` as the session/catalog/patch
  owner;
- the runtime-driver entry API is explicit enough that new entries bind to the
  committed active generation without relying on a one-off restart method;
- real AWFB/patch smoke fixtures validate windowed live-patch behavior;
- a local/dev ingress adapter can enqueue typed patch events without directly
  mutating runtime state.

The first two items are complete as of seq03.5/seq03.6. Until seq03.7 and
seq03.8 land, seq04 may be designed in parallel but seq03 is not fully closed.
