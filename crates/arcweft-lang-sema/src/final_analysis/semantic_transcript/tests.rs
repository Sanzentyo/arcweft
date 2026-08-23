use super::*;

fn assert_pairwise_unique(tags: &[u8]) {
    for (index, tag) in tags.iter().enumerate() {
        assert!(!tags[..index].contains(tag), "duplicate tag {tag}");
    }
}

fn checked_path_digest(step: CheckedExpressionChildRoleStep) -> [u8; 32] {
    let path = CheckedExpressionChildRolePath::new(
        AcceptedDeclarationSemanticId::from_bytes([0xa5; 32]),
        [step],
    );
    let mut used = 0;
    let mut hasher = TranscriptHasher::new(&mut used, u64::MAX);
    write_checked_path(&mut hasher, &path);
    hasher.finalize().expect("bounded checked path")
}

fn hir_local_path_digest(step: HirSemanticPathStep) -> [u8; 32] {
    let mut used = 0;
    let mut hasher = TranscriptHasher::new(&mut used, u64::MAX);
    write_hir_local_path(&mut hasher, &[step]).expect("bounded HIR-local path");
    hasher.finalize().expect("bounded HIR-local path")
}

#[test]
fn checked_path_step_tags_are_append_only_and_payload_sensitive() {
    assert_pairwise_unique(&[
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        CHECKED_DECLARATION_BODY_STEP_TAG,
        CHECKED_EXPRESSION_OWNED_STEP_TAG,
    ]);
    assert_eq!(CHECKED_DECLARATION_BODY_STEP_TAG, 8);
    assert_eq!(CHECKED_EXPRESSION_OWNED_STEP_TAG, 9);
    assert_ne!(CHECKED_DECLARATION_BODY_STEP_TAG, 7);

    let declaration = checked_path_digest(CheckedExpressionChildRoleStep::DeclarationBody(
        HirDeclarationBodyRootRole::FunctionBody,
    ));
    let owned_zero = checked_path_digest(CheckedExpressionChildRoleStep::ExpressionOwned(
        HirExpressionOwnedBodyRole::AwaitBranchPattern { branch: 0 },
    ));
    let owned_one = checked_path_digest(CheckedExpressionChildRoleStep::ExpressionOwned(
        HirExpressionOwnedBodyRole::AwaitBranchPattern { branch: 1 },
    ));
    let prior = checked_path_digest(CheckedExpressionChildRoleStep::ThreadBody(
        HirStatementBodyRole::LetElse,
    ));
    assert_ne!(declaration, owned_zero);
    assert_ne!(declaration, prior);
    assert_ne!(owned_zero, prior);
    assert_ne!(owned_zero, owned_one);
}

#[test]
fn hir_local_path_step_tags_are_append_only_and_payload_sensitive() {
    assert_pairwise_unique(&[
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        HIR_LOCAL_DECLARATION_BODY_STEP_TAG,
        HIR_LOCAL_EXPRESSION_OWNED_STEP_TAG,
    ]);
    assert_eq!(HIR_LOCAL_DECLARATION_BODY_STEP_TAG, 7);
    assert_eq!(HIR_LOCAL_EXPRESSION_OWNED_STEP_TAG, 8);

    let declaration = hir_local_path_digest(HirSemanticPathStep::DeclarationBody(
        HirDeclarationBodyRootRole::ViewValue { ordinal: 0 },
    ));
    let declaration_next = hir_local_path_digest(HirSemanticPathStep::DeclarationBody(
        HirDeclarationBodyRootRole::ViewValue { ordinal: 1 },
    ));
    let owned = hir_local_path_digest(HirSemanticPathStep::ExpressionOwned(
        HirExpressionOwnedBodyRole::AwaitBranchBody { branch: 0 },
    ));
    let prior = hir_local_path_digest(HirSemanticPathStep::ThreadBody(
        HirStatementBodyRole::LetElse,
    ));
    assert_ne!(declaration, declaration_next);
    assert_ne!(declaration, owned);
    assert_ne!(declaration, prior);
    assert_ne!(owned, prior);
}
