# Retained parent substrate

The following parent decisions remain normative without redesign:

- canonical Serde `RuntimeValuePath` in `value::ownership::path`, including
  `OpaquePayload` tag `10` for physical ownership/save/diagnostic traversal;
- distinct non-Serde `RuntimeCheckedTypePath`;
- checked path push rules for Sequence, Bytes, Tuple, Choice, Result, Option,
  nominal/closed Variant, and nominal records, except that the checked opaque
  row is replaced by atomic semantics in this package;
- complete `RuntimeValueShape` classification and descriptor-sourced nominal
  identity comparison;
- lossless 32-byte semantic-ID projection to distinct project and producer root
  newtypes;
- typed plan/AWBC coordinates and nested tags except the audio coordinate
  explicitly replaced here;
- direct plan-to-AWBC origin equality without another digest/root map;
- one immutable admitted-generation parent and non-Serde wrappers;
- project-versus-producer nominal construction domains and version-1 AWBC
  `MakeRecord` operands;
- lower `CharacterDialogueRuntimeRole` vocabulary, accepted-world role
  issuance, and canonical CharacterCatalog/ViewRegistry/custom digests.

Copied retained tables are archive members. The corrected versions of
`CHECKED_PATH_PUSH_RULES.csv`, `AWBC_SITE_RESOLUTION.csv`,
`AWBC_REFERENCE_RESOLUTION.csv`, `AWBC_SITE_CANONICAL_TAGS.csv`, and
`TEST_MATRIX.csv` in this archive supersede their parent copies.
