# Lang-01.1.1.3 effect-trait return intake

Date: 2026-07-25

## Package verification

The returned archive is retained at:

- [`arcweft-lang-01.1.1.3 effect-trait reconciliation`](../reviews/packages/zips/arcweft-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation-final-contract.zip),
  SHA-256
  `4FD834564C458639CD4EBE46615E4EC79C54F91D686439AAAACCC7F2B3714B5E`,
  46,663 bytes.

The ZIP has 13 members. Every hash and 12-digit byte length in its 12-row
`MANIFEST.sha256` matches the corresponding non-manifest member.
`OPEN_QUESTIONS.md` is exactly `none`; the embedded request hash
`65B912D18765C24FCAD7F195EF4A6914992FD28B220EC4FC11043E04E9EE7330`
matches the repository request, and the parent archive hash recorded by the
package matches the supplied parent. All status, summary, evidence, matrix, and
ownership sidecars are inside the ZIP; no adjacent sidecar is required.

The package inspected pushed Git commit
`0b7e095f4193b9f7fbbc95cc350a626a8a63640a`. Its local Jujutsu change is
`pxulxlkmwqztnrwykmtowvvlkruusooy`; the package truthfully omitted that value
because GitHub did not expose the Jujutsu header.

## Intake status

`RETURNED_SEMANTICS_ACCEPTED_CATALOG_AND_TRAIT_VALIDATOR_CORRECTIONS_RETURNED`.

The following design decisions are accepted and do not need to be requested
again:

- parent E017 is superseded for Lang-01.1.1 and dynamic trait objects remain a
  future language feature;
- E017S covers only the supported static-witness path;
- omitted bodyless trait effects are the real closed empty row;
- implementation rows use the existing typed row/substitution model and exact
  subset checking;
- E015/E016/E022/E023 use one typed diagnostic object and revision-bound spans
  for CLI/LSP;
- method values and static witnesses retain typed requirement/implementation
  identity; and
- `TraitCallableId`, empty resolver rows, string effect identities, copied
  requirement rows, generic `AWF-EFX-001`, project method-value rejection,
  string/local-index runtime identity, and witness-plus-method-name lookup are
  deleted in the final authority switch.

No `dyn` parser placeholder, compatibility alias, dual reader, source gate,
removed-syntax diagnostic, CSS path, or Takumi path is authorized.

## Returned catalog and trait-validator closures

The catalog-authority correction has returned and is recorded in the
[Lang-01.1.1.3.1 intake](2026-07-25-lang-01-1-1-3-1-checked-callable-catalog-intake.md).
It selects retention of the exact accepted `Arc<CallableRecord>` and delegates
all accepted metadata reads from checked facts. That closes the former copied
signature/source authority gap and must not be reopened.

The narrower identity boundary has also returned. It replaces
`CallableValidator::Trait(TraitCallableId)` with the role-only
`CallableValidator::Method(CallableMethodRole)`, keeps exact identity in the
accepted record and checked shell, and assigns the observational
`CallableFamily::TraitMethod` projection to `CallableRecord::family()`.

Its verified intake is:

- [Lang-01.1.1.3.1.1 trait validator and resolver-family return intake](2026-07-25-lang-01-1-1-3-1-1-trait-validator-resolver-family-intake.md).

## Current implementation WIP disposition

The uncommitted ordinary-callable consumer migration is preserved, but it is
not treated as the final Lang-01.1.1.3 public authority.

Stable substrate that may be separated into an independent coherent cut:

- exact structural `CallableDeclarationId` joins, which the returned package
  itself retains inside `CallableDeclarationKey::Existing`;
- accepted `Arc<CallableRecord>` signature lookup and semantic digest;
- accepted LSP snapshot/source-revision validation; and
- persistent interface failure on missing, foreign, or stale accepted catalog
  records instead of raw-HIR signature fallback.

Work held for the corrected Lang-01.1.1.3 authority switch:

- pre-check trait/inherent validator identity and the trait-method observational
  family projection;
- project graph/callable public keys that must become revision-bound checked
  IDs;
- copied `ProjectFunctionSymbol` effect rows;
- Agent declaration/effect projections derived from those copied rows; and
- LSP lookups that would publish the structural ID as the final checked
  authority.

Deletion-driven migration remains mandatory: old raw-HIR/name/effect readers
are not repaired or reintroduced while the WIP is split.

## Production boundary

Both corrections have returned and are implementation-ready. The final
Lang-01.1.1.3 public authority switch remains at its established dependency
position after the active Proof and typed RichText work; this ordering is no
longer a design wait. The accepted Stream wire correction remains independent.
