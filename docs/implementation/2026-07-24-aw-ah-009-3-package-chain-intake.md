# AW-AH-009.3 signature-help package-chain intake

Date: 2026-07-24

## Intake result

The complete accepted AW-AH-009.3 package chain is now retained under
`docs/reviews/packages/` before the remaining production matrix is changed.
Every archive was read from the protected integration change
`zzrlxnsunyxl`, hashed again from the current checkout, reopened, and checked
against its internal manifest.

| Sequence | Archive SHA-256 | Verified content members | Status | Normative role |
| --- | --- | ---: | --- | --- |
| AW-AH-009.3 | `cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5` | 10/10 | ready, no open questions | Base native Character nominal signature-help query, identity/cache/limits, surface inventory, and full matrix |
| AW-AH-009.3.1 | `6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588` | 9/9 | ready, no open questions | Exhaustive authored call-surface syntax; parenthesized versus callback-block ownership |
| AW-AH-009.3.2 | `8701ff3ae6024cd62c33c4b36abdfa358bfa30aa93209655870c475eea1dd40d` | 9/9 | ready, no open questions | Accepted HIR/source/module lease, request control, cache publication, and accepted-build limits |
| AW-AH-009.3.3 | `9d1f989f5e0e698aeff1098dd7ecee7e01a66616a00a0571ee333a3b1b7ddc78` | 10/10 | ready, no open questions | One typed callable catalog/resolver and checker-owned call-target facts |
| AW-AH-009.3.3.1 | `3d81158eb37f503ef7b0f242a79015ba1ab00e3954a8dae4384f45eaab55b672` | 10/10 | ready, no open questions | Curried callable group validation at the resolved-callable success boundary |
| AW-AH-009.3.3.2 | `c5b6bbf9addb45f2d6ecbdfd8f2abc4d6602f079a847a20db8f26140d53a248f` | 13/13 | ready, no open questions | Typed external project-binding path publication through HIR, project symbols, adapters, and the callable catalog |

There were no member digest or declared-byte-length mismatches. The base
package and every correction report `OPEN_QUESTIONS.md` as `none` and a ready
implementation status.

## Precedence and implementation use

The base AW-AH-009.3 contract remains normative except where a later numbered
correction directly replaces one of its production assumptions. The
corrections are consumed in numeric order:

1. use the AW-AH-009.3.1 exhaustive call-surface enum rather than assuming
   that every semantic call owns parentheses;
2. acquire source/HIR/world only through the AW-AH-009.3.2 accepted lease and
   request lifecycle;
3. resolve checker and signature-help calls through the AW-AH-009.3.3 shared
   catalog/resolver;
4. apply AW-AH-009.3.3.1 curried-group validation at
   `ResolvedCallable::try_new`; and
5. retain AW-AH-009.3.3.2 typed segmented external binding paths without
   parsing display strings.

The package chain does not depend on Proof-concurrency syntax identity.
`SourceDocumentIdentity` and parser-retained exact typed call ranges remain the
signature-query source identity. `SourceSnapshotId` must not be accepted or
converted into that identity merely to satisfy a test.

## Current implementation boundary

Substantial substrate is already present on `main`: the shared callable
catalog/resolver, accepted project lease, semantic overload/resource/work
accounting, exact typed Character nominal results, and a bounded native cache.
The remaining cut is an evidence-driven matrix closure. It must add missing
compile-fail identity boundaries and production exact/one-over limit evidence,
reconcile any remaining surface/shadow/cache rows, run the required focused
and workspace gates, and avoid carrying unrelated Proof, RichText, Stream,
resource, TTS, or CharacterDialogue runtime changes into the same commit.

## Prohibited shortcuts

No source gate, source-text search, signature-only resolver, display-label
parser, compatibility alias, dual reader, removed-syntax recognizer, CSS path,
or Takumi path may be introduced. Limits must be tested through production
owners rather than only through reduced custom meters when the contract names
an exact production boundary.

This intake cut changes no Rust, Cargo manifest, schema, fixture, or production
behavior.
