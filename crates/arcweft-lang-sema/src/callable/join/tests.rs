// The former tests exercised deleted pre-final join helpers.  Final join
// validation is covered through the prepared application integration tests.

#[test]
fn content_candidate_has_a_distinct_intrinsic_join_tag() {
    let candidate = crate::callable::CallableCandidateId::Content(
        crate::callable::ContentCallableIdentity::language(
            arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Strong,
            arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                .get(
                    arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Strong,
                )
                .expect("Strong content row")
                .schema_digest(),
        ),
    );
    let tag = super::IntrinsicCallableCandidateTag::from_candidate(&candidate)
        .expect("content is an intrinsic candidate");
    assert_eq!(tag, super::IntrinsicCallableCandidateTag::Content);
    assert_eq!(tag.semantic_tag(), 22);
}
