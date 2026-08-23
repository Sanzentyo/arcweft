//! Rust-shaped review contract. This file is documentation, not production code.
#![allow(dead_code)]

pub const CONTRACT_VERSION: u8 = 1;

pub struct Digest32(pub [u8; 32]);
pub struct SemanticTypeDigest(pub Digest32);
pub struct StableExpressionCoordinate(pub Digest32);
pub struct StablePatternCoordinate(pub Digest32);
pub struct StableMatchArmCoordinate(pub Digest32);

pub struct AcceptedProjectItemSemanticId(pub Digest32);
pub struct AcceptedVariantCaseSemanticId(pub Digest32);
pub struct AcceptedRecordFieldSemanticId(pub Digest32);
pub struct AcceptedEnvironmentFieldSemanticId(pub Digest32);
pub struct AcceptedCharacterLookSemanticId(pub Digest32);
pub struct AcceptedViewModifierSemanticId(pub Digest32);
pub struct CheckedExpressionSemanticDigest(pub Digest32);
pub struct CheckedPatternSemanticDigest(pub Digest32);
pub struct CheckedStatementSemanticDigest(pub Digest32);
pub struct CheckedBodySemanticDigest(pub Digest32);
pub struct CheckedRichTextSemanticDigest(pub Digest32);
pub struct MatchSemanticDigest(pub Digest32);

pub enum CheckedGuardSemantic {
    Absent,
    ConstantTrue,
    ConstantFalse,
    Dynamic(CheckedExpressionSemanticDigest),
}

pub struct CheckedMatchArm {
    pub coordinate: StableMatchArmCoordinate,
    pub pattern: CheckedPatternSemanticDigest,
    pub guard: CheckedGuardSemantic,
    pub result: CheckedExpressionSemanticDigest,
}

pub struct CheckedMatchCoverage {
    pub exhaustive: bool,
    pub unreachable: Box<[CheckedUnreachablePattern]>,
    pub witness: Option<CheckedCoverageWitness>,
    pub stats: CheckedCoverageStats,
}

pub struct CheckedUnreachablePattern {
    pub arm: StableMatchArmCoordinate,
    pub alternative: Option<StablePatternCoordinate>,
    pub reason: CheckedUnreachableReason,
}

pub enum CheckedUnreachableReason {
    CoveredByPriorUsefulArms,
    CoveredByEarlierOrAlternative,
    ConstantFalseGuard,
    UninhabitedDomain,
}

pub enum CheckedCoverageWitness {
    Unit,
    Bool(bool),
    Literal(Digest32),
    Entity(AcceptedProjectItemSemanticId),
    Other(SemanticTypeDigest),
    Variant {
        case: AcceptedVariantCaseSemanticId,
        payload: Option<Box<CheckedCoverageWitness>>,
    },
    Tuple(Box<[CheckedCoverageWitness]>),
    Record {
        owner: SemanticTypeDigest,
        fields: Box<[CheckedCoverageWitness]>,
    },
    Array(Box<[CheckedCoverageWitness]>),
    Sequence {
        partition: CheckedSequencePartitionWitness,
        visible_prefix: Box<[CheckedCoverageWitness]>,
    },
    Choice {
        ordinal: u32,
        alternative: SemanticTypeDigest,
        value: Box<CheckedCoverageWitness>,
    },
}

pub enum CheckedSequencePartitionWitness {
    Exact(u64),
    Interval {
        lower: u64,
        upper_exclusive: Option<u64>,
    },
}

pub struct CheckedCoverageStats {
    pub matrix_rows: u64,
    pub specializations: u64,
    pub sequence_partitions: u64,
    pub witness_nodes: u64,
}

pub struct CheckedMatchLimits {
    pub max_arms: u64,
    pub max_matrix_rows: u64,
    pub max_or_alternatives: u64,
    pub max_pattern_nodes: u64,
    pub max_expression_nodes: u64,
    pub max_depth: u64,
    pub max_sequence_partitions: u64,
    pub max_specializations: u64,
    pub max_unreachable_rows: u64,
    pub max_witness_nodes: u64,
    pub max_transcript_bytes: u64,
}

pub enum CheckedMatchLimitKind {
    Arms,
    MatrixRows,
    OrAlternatives,
    PatternNodes,
    ExpressionNodes,
    Depth,
    SequencePartitions,
    Specializations,
    UnreachableRows,
    WitnessNodes,
    TranscriptBytes,
}

pub enum CheckedMatchBuildError {
    MissingExactOwner,
    PoisonedSemanticNode,
    DuplicateSemanticPath,
    InvalidCheckedRow,
    UnsupportedDomain(SemanticTypeDigest),
    LimitExceeded {
        kind: CheckedMatchLimitKind,
        limit: u64,
        attempted: u64,
    },
    ArithmeticOverflow(CheckedMatchLimitKind),
}

pub enum HirDeclarationBodyRootRole {
    FunctionBody,
    PredicateBody,
    ProofBody,
    FlowBody,
    ImplFunctionBody,
    ViewValue { ordinal: u32 },
}

pub enum HirExpressionOwnedBodyRole {
    AwaitBranchPattern { branch: u32 },
    AwaitBranchBody { branch: u32 },
    ChoiceLetStatement { item: u32 },
    ChoiceForPattern { item: u32 },
    ChoiceMatchArmPattern { item: u32, arm: u32 },
    ChoiceOptionForPattern { item: u32 },
    ChoiceOptionSelectBody { item: u32, field: u32 },
    ChoiceOptionLetStatement { item: u32, field: u32 },
    ChoicePlanTimeoutBody { item: u32 },
    ChoicePlanCancelBody { item: u32 },
    ChoicePlanOnSelectPattern { item: u32 },
    ChoicePlanOnSelectBody { item: u32 },
    DialogueLinePlanStatement { group_path: Box<[u32]>, item: u32 },
    DialogueLinePlanLetPattern { group_path: Box<[u32]>, item: u32 },
}
