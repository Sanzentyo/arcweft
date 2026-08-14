# Checked-value/root-site return intake and residual blocker

Date: 2026-08-14

Continues:
`docs/implementation/2026-08-14-catalog-authority-retry-intake-and-residual-blocker.md`

Inspected Git baseline:
`eb450570acff118ccc3e2a75751144f037af170f` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned archive intake

The downloaded archive had a delivery-only `1-` prefix. Because the canonical
repository name was free, intake removed that prefix rather than preserving a
collision marker. The retained archive is:

`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1-checked-value-path-and-resolvable-root-site-correction-final-contract.zip`

SHA-256:
`b8a2d1d5e09ad21c5372af11454f3f22188046d3af6bafc4637e0446c2cd531b`

The 161,948-byte ZIP contains 76 files without a redundant wrapper. It has no
unsafe/rooted/drive/traversal path, symlink/reparse entry, or case-insensitive
collision. All 76 extracted files match their ZIP-member SHA-256 values and
all 75 internal `MANIFEST.sha256` rows pass. `SOURCE_REQUEST.md` is
byte-identical to the maintained request, SHA-256
`034eb287c315d699d1cf110babaffbd80650d2b8c1eb340bb6e8d6b6efc6c32e`.

The package reports `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, evidence
commit `36f83f8509417d1110a34f1b32aee6f4a113dcf3`, and Arcweft-owned versions
fixed at `1`. Its 12 decisions, typed-site declarations, mapping tables,
772-row inventory, 1,878-row test matrix, and 43 source-evidence rows materially
answer most of the maintained correction. Of those 43 source rows, 41 still
match the current baseline. The two changed files contain the subsequently
accepted lower role-vocabulary and View-revision migrations and do not explain
the blockers below.

## Readiness adjudication

Full-package/current-source inspection and an independent Sol max audit
classify the return as `INVALID_AS_DELIVERED`. The defects are internal,
current-Git-resolvable contract contradictions, not external unresolved
authority, so `NOT_READY` is not the correct repository classification.

1. The package places the final typed plan and AWBC owners in
   `arcweft-core`, makes their fields private and checked constructors
   `pub(crate)`, then requires the separate `arcweft-runtime-plan` crate to
   construct them. Rust has no friend-crate visibility. This affects
   `RuntimePlanTypeId`, declarations, `RuntimeTypedExpr`,
   `RuntimeTypedPattern`, typed AWBC origins/constants/patterns, and the final
   RuntimePlan/AWBC aggregate construction boundary. The requested
   compile-fail tests simultaneously prohibit the public construction path the
   real lowerer needs.
2. Decision 6 says accepted semantic HIR and producer facts are non-Serde
   inputs assembled before raw-artifact admission. The normative API instead
   makes `RuntimePlan::try_admit(self)` and `AwbcProgram::try_admit(self)`
   construct the one admitted generation from each artifact's serialized
   generation declaration. It never supplies the independent accepted facts
   needed to populate or verify the generation. A raw artifact can therefore
   become the source of the authority against which its own declarations are
   checked.
3. The retained parent owns `RuntimeProjectRootFact` and
   `RuntimeProducerFact` in `arcweft-runtime-plan`, while the returned
   `AdmittedRuntimeGenerationInner` is owned by `arcweft-core` and stores those
   fact types. The lower core crate cannot name an upward-owned type. No final
   lower-layer projection, issuance token, or atomic lowering/admission API is
   defined.
4. `RuntimeCheckedType::Opaque` stores only its exact/wide owner, while the
   returned validator requires an `OpaquePayload` descent and says to validate
   a producer payload contract. Neither the returned package nor the retained
   root fact gives an exact owner-to-payload checked type lookup and error
   precedence. Current production validates only the opaque owner. Guessing
   atomic versus recursive payload admission changes accepted values and error
   evidence.
5. `AWBC_SITE_RESOLUTION.csv` cannot independently resolve AudioCommand rows.
   `AwbcTypedSite::AudioCommand { command, slot }` lacks the owning EffectPlan
   coordinate, one command may be referenced by multiple effects, and the CSV
   also requires `EffectPlan::AudioValue(slot)` while
   `AWBC_SLOT_ENUMS_AND_TAGS.md` explicitly prohibits that variant.
6. `RuntimeTypedExpr` requires one complete root `[0]` fact set, while the
   expression mapping deliberately excludes function, range, matrix, tensor,
   and other values with no representable `RuntimeCheckedType`. The contract
   does not select rejection, an expanded closed type, or a final explicit
   untyped-node rule.
7. The corrected phase order is still circular. It issues
   `RuntimeNominalRecordAdmissionDomain::checked_values()` before the
   `AdmittedRuntimePlan` and typed plan sites that issue the project-scoped
   domain exist. The promised final-owner-only compile-clean stages therefore
   cannot be followed as written.
8. Normative tests contradict the selected behavior. AudioCommand rows are
   tested as exclusions although the site CSV makes them mandatory, and tests
   require child/index failures for payload-free Option `None` and for edges
   with no numeric index. The package therefore cannot meet its own acceptance
   matrix.

## Safe implementation boundary

The accepted lower role vocabulary and Character/View canonical digests remain
valid and are already implemented on current `main`. The retained parent's
lossless `RuntimeSemanticTypeId` to project/producer root-ID projection is an
independent scalar cut. Do not implement typed plan/AWBC DTOs, admitted
generation construction, raw admission, checked opaque recursion, pair
correlation, or execution migration until the child correction closes the
listed authority boundaries.

Child correction request:
`docs/reviews/requests/2026-08-14-lang-01.3.1.2.3.2.1.2.1.1.1.1-external-lowering-and-independent-generation-admission-authority-correction.md`

## Validation performed

- source ZIP SHA-256 and byte length: verified;
- unsafe path, traversal, symlink/reparse, and case-collision preflight: passed;
- ZIP member versus extracted file SHA-256 parity: 76/76 passed;
- internal `MANIFEST.sha256`: 75/75 passed;
- request-copy SHA-256 equality: passed;
- all normative Markdown, decisions, mapping CSVs, metadata, inventory, tests,
  and repository evidence were inspected; and
- no blocked plan/AWBC/admission production code was implemented.
