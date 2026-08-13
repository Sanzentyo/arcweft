# Nominal runtime-value A4 dialogue-authority blocker

Date: 2026-08-12

Inspected Git baseline:
`98ccafa5f0113a50f8a0f5e985df5f695c401588` on `main`, equal to
`origin/main`, with a clean working tree.

## Established state

The selected nominal-record package's A1-A3 gates are implemented. The A4
core value owner already has crate-private checked
`try_from_accepted_layout`, public `validate_against_layout`, field-ID queries,
and typed identity/layout/count/field validation. Public unchecked
`RuntimeNominalRecordValue::new` and `validate_shape` still exist because all
external consumers cannot yet be migrated safely.

The blocking production evidence is in `arcweft-dialogue`:

- `CharacterDialogueRuntimeSchema` receives only a `TypeLayoutHash`, not a
  complete active `RuntimeNominalRecordLayout`;
- fixed custom-entry and inline-failure records have identity/layout hashes
  but no closed checked field descriptor;
- the custom-entry schema uses a dynamic named payload that cannot be
  represented by the current closed `RuntimeCheckedType`; and
- arbitrary nested nominal records are reconstructed during normalization,
  clearing, and structured patching without a descriptor that could validate
  the changed fields.

Making the checked constructor public would expose arbitrary nominal/layout
minting. Rejecting every descriptorless nominal transform would be fail-closed
but would change currently accepted CharacterDialogue behavior. Sol max was
used for this result-changing judgment and confirmed that neither choice is a
safe package-local implementation inference.

## Required correction

Production A4 work is stopped before deleting the unchecked constructor. The
independently throwable correction request is:

`docs/reviews/requests/2026-08-12-lang-01.3.1.2.3.2.1.2-nominal-runtime-value-external-admission-and-dialogue-layout-authority-correction.md`

It asks only for the external checked admission authority, complete
CharacterDialogue layout ownership, descriptorless nested-nominal transform
semantics, ingress validation, and deletion order. It fixes all Arcweft-owned
versions at `1` and does not reopen A1-A3, opaque ownership, activation, View,
or Stream decisions.

No A4 production code or compatibility path was added in this blocker cut.

## 2026-08-13 returned-package adjudication

The `.1.2` archive returned and was integrity-verified, but its proposed
producer authorization and independent AWBC execution authority remain
underdesigned. The current status and the narrower `.1.2.1` request supersede
this note as active blocker evidence:

`docs/implementation/2026-08-13-nominal-runtime-value-authority-package-intake-and-blocker.md`
