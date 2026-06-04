/// Canonical Arcweft LSP command identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcweftCommand {
    ExpandSugar,
    MaterializeId,
    GenerateProofStub,
    GenerateUnsafeAudit,
    ShowObligation,
    NavigateToProof,
    NavigateToUnsafeAudit,
}

impl ArcweftCommand {
    /// Stable command id sent over LSP.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpandSugar => "arcweft.expandSugar",
            Self::MaterializeId => "arcweft.materializeId",
            Self::GenerateProofStub => "arcweft.generateProofStub",
            Self::GenerateUnsafeAudit => "arcweft.generateUnsafeAudit",
            Self::ShowObligation => "arcweft.showObligation",
            Self::NavigateToProof => "arcweft.navigateToProof",
            Self::NavigateToUnsafeAudit => "arcweft.navigateToUnsafeAudit",
        }
    }

    /// All command ids advertised by the MVP server.
    pub const fn all() -> [Self; 7] {
        [
            Self::ExpandSugar,
            Self::MaterializeId,
            Self::GenerateProofStub,
            Self::GenerateUnsafeAudit,
            Self::ShowObligation,
            Self::NavigateToProof,
            Self::NavigateToUnsafeAudit,
        ]
    }

    /// Parses a command id received from a client.
    pub fn parse(value: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|command| command.as_str() == value)
    }
}
