# Constraints and non-goals

The following are explicitly prohibited:

- making `RuntimeNominalRecordValue::try_from_accepted_layout` public;
- adding a public raw nominal/layout/fields value constructor;
- treating `TypeLayoutHash`, nominal name, schema name, semantic digest, or
  source field names as a recoverable field descriptor;
- `RuntimeCheckedType::Dynamic`, optional validation, producerless opaque
  ownership, or descriptorless success;
- a dialogue-only unchecked constructor, friend Cargo feature, extension trait,
  copied descriptor table, global producer callback registry, source-string
  resolver, or post-build overlay;
- preserving root/custom/inline nominal and new opaque/tuple representations in
  parallel;
- preserving public `new`, `validate_shape`, `RuntimeFieldPath`, or direct live
  typed-value Deserialize behind aliases/deprecation;
- failing all nominal normalize/clear/patch operations when an active
  descriptor exists, or silently succeeding without one;
- walking ownership/nesting before active validation;
- changing accepted A1–A3 nominal identity/layout/expression/anonymous carrier/
  field-ID rules without a concrete separately reviewed defect;
- redesigning affine ownership/slots, activation-domain identity, final HIR
  View products, Stream publication, or unrelated error models;
- allocating any schema, ABI, codec, digest-domain, protocol, replay, save, or
  bundle version; and
- including production source, patch, branch, PR, implementation overlay, or
  compatibility artifact in this archive.

The closed-variant `RuntimeCheckedType::accepts_value` correction is in scope
because the pinned inherent implementation's owner-only nominal variant branch
cannot enforce the exact inline-failure case contract required here. The
behavior is added to the original enum implementation, not routed through an
ad-hoc helper.
