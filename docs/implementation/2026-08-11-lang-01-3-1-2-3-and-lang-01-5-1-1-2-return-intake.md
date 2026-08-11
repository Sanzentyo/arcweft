# Lang-01.3.1.2.3 and Lang-01.5.1.1.2 return intake

Date: 2026-08-11

Inspected clean Git baseline:
`e619231de8fe0e7c2a9d0d7be15a3608be042058` on `main`, equal to
`origin/main`, with one checkout and no additional worktrees.

Supersedes the active external-design blockers recorded in:

- `2026-08-08-lang-01-3-affine-runtime-value-substrate-blocker.md`; and
- `2026-08-10-lang-01-5-1-1-2-final-hir-view-execution-boundary-blocker.md`.

Those notes remain historical implementation evidence. This intake supersedes
only their waiting status, not their production observations or non-goals.

## Returned archives

The following attached archives were copied without modification to
`docs/reviews/packages/` after inspection:

| Sequence | Bytes | SHA-256 | Classification |
|---|---:|---|---|
| Lang-01.3.1.2.3 | 191,321 | `d053fae201afa104f7db9914aebbc08f2456875d1229f5325f86235d4bc0ea94` | implementation-ready subject to the mandatory correction below |
| Lang-01.5.1.1.2 | 87,232 | `87b7f7bea85bc54254e3a979f0d668026ab75cb1c71955fd7a0f740e4f30c1c6` | implementation-ready subject to the mandatory correction below |
| Lang-01.3.1.2.3.1 | 53,925 | `a52453fd07fdacf10205cbf621077f923ded714b83e4c64b9b69c52a7350ff7f` | accepted mandatory correction of both parent contracts |

The affine and HIR View `SOURCE_REQUEST.md` members match the corresponding
repository requests byte-for-byte by SHA-256. All three
`OPEN_QUESTIONS.md` members are exactly `none\n`, and all three status files
report `READY_FOR_IMPLEMENTATION` with zero result-changing decisions open.

## Integrity performed

- Archive SHA-256 and byte length: passed for all three returned ZIPs.
- Safe member enumeration and extraction: passed.
- Affine parent `MANIFEST.txt`: 28 payload hashes passed; the documented
  all-zero self row was skipped.
- ABI1/View correction `MANIFEST.txt`: 34 payload hashes passed; the documented
  all-zero self row was skipped.
- HIR View `SHA256SUMS`: all 35 recorded member hashes passed.
- Existing retained review-package inventory: 34 ZIPs enumerated before this
  intake; their byte lengths and SHA-256 values were recomputed without finding
  a name collision with the three returned archives.

The package-local Python reference models and validators are reported as passed
by their retained validation reports. They were not rerun during this intake;
they are design-package evidence and do not claim Arcweft production validation.

## Current-main reconciliation

The HIR View package inspected production baseline
`a6805f7375499e5cce70f84f1531832583474527`. The only later commit at intake is
the documentation-only request commit `e619231de8fe0e7c2a9d0d7be15a3608be042058`;
there is no intervening `crates/` diff.

The affine package names baseline
`177ba1e61e43fb2da2149869ce35e165d1e93b66`. The generic affine owners in
`arcweft-core` and `arcweft-runtime-plan` have not changed since that baseline.
Later production changes are the accepted resource-manifest cut in bundle,
runtime-driver, and runtime-host. The mandatory correction package used the
exact delivered affine and HIR View parent ZIP hashes and the observed
request-only main as inputs. No current accepted production fact changes a
result selected by the corrected contracts.

Accepted P3 typed external project-binding evidence remains present through the
`ProjectDirectBinding` owner and its HIR/sema tests. No `StreamHandle`,
`RuntimeStreamDefinition`, or external Stream partial is constructible in
production, so the pre-P4+C1 safety condition remains satisfied.

## Accepted precedence and dependency order

Lang-01.3.1.2.3.1 supersedes only its explicit parent rows. In particular:

- `AWBC_ABI_VERSION` remains 1 and codec 8 is directly replaced with the final
  ownership-complete ABI1 meaning;
- generic affine ownership, exact capture, payload, plan-constant, sequence,
  snapshot, and Stream-parent decisions otherwise remain authoritative;
- current retained/render View values must be statically `Unrestricted`, while
  handler input is moved exactly once;
- View value-product inputs carry exact ownership and transfer intent;
- save schema 2 uses whole-execution dormant value snapshots rather than live
  `RuntimeBinding` serialization; and
- serialized static requirements, strict fragment containment, and outermost
  fragment dispatch correct the HIR View parent.

Production order is:

1. freeze and add the generic ownership substrate while no affine leaf is
   constructible;
2. migrate structured runtime, plans, AWBC/fibers, snapshot, and all executable
   Clone/Serde consumers through the final checked owner;
3. publish the corrected View ownership facts and final-HIR catalog/product
   cuts against that owner;
4. publish Stream handles/partials only after unconditional executable Clone is
   physically gone; and
5. complete host, bundle, save, hot-reload, parity, Tier 2, and structural gates.

The first production cut is the additive generic classification/path/error and
checked unrestricted traversal boundary. It does not touch View yet, add a token
constructor, publish a Stream handle, change AWBC bytes, or retain a compatibility
alias.

## Validation state

Performed and passed for intake: Git identity/state, request identity, ZIP and
member integrity, baseline source-delta comparison, P3 owner presence, and
absence of a constructible Stream runtime owner.

Not run for this documentation intake: Cargo format/check/Clippy/tests, Tier 2,
generated-artifact comparison, and structure audit. Production validation will
be recorded at each corrected implementation cut.
