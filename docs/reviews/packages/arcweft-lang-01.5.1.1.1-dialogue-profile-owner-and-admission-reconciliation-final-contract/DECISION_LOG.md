# Decision log

## D1 — preserve launch ownership

**Selected:** `DialogueProfileSpec`, sole decoder, `SourceBackedManifest`, and
generic source map remain in `arcweft-launch`.

**Rejected:** presentation dependencies in `arcweft-manifest-model`.

**Reason:** neutral lower consumers must not import View/dialogue presentation
ownership; current source already has the cycle-free direction.

## D2 — admit after compiler product construction

**Selected:** compiler transaction after one immutable validated View/Style
product exists.

**Rejected:** project-loader admission against runtime-driver catalog; second
lower catalog.

**Reason:** project-loader does not compile View programs; runtime catalog is
constructed later and has private capabilities. Reversal or duplication would
violate layers and create competing truth.

## D3 — project the existing source map

**Selected:** typed `ManifestTokenPath`/`ManifestTokenSlot` access over the one
revision-bound map.

**Rejected:** dedicated dialogue source records/map or second scan.

**Reason:** all ranges must share the accepted document identity and decoder
transaction.

## D4 — retain exact wire and policy

**Selected:** `inline-failure` plus dialogue-owned strict tagged enum.

**Rejected:** `inline_failure`, changed fallback shape/spellings, bridge enum,
alias, dual reader.

**Reason:** current schema is strict kebab-case and the dialogue owner already
provides the canonical policy codec.

## D5 — split semantic and physical ownership correctly

**Selected:** compiler owns admission/checked profile; dialogue owns the lower
six-field revision value.

**Rejected:** runtime-plan importing a compiler-owned revision.

**Reason:** compiler already depends on runtime-plan. A reusable compiler-owned
value would force a cycle. Moving only the immutable lower value preserves
semantics without a conversion shim.

## D6 — exact candidate equality and Arc identity

**Selected:** structural equality over all six revision facts plus admission-time
resource-registry Arc identity and exact retained product Arc.

**Rejected:** ID-only or digest-only compatibility.

**Reason:** atomic publication must never combine independently accepted
manifest, source, program, or resource states.

## D7 — delete old source authority

**Selected:** direct deletion of `dialogue defaults` and `@dialogue.*` paths.

**Rejected:** compatibility parser, removed-syntax diagnostic, feature gate,
dual success path.

**Reason:** unreleased internal authority is replaced directly; ordinary parser
recovery is sufficient.

## D8 — status is resolved/as-built

**Selected:** design remains implementation-ready but current main is already
implemented; do not redispatch the historical request.

**Rejected:** claiming an open blocker or inventing new implementation work.

**Reason:** source request and maintained implementation records explicitly
close it.
