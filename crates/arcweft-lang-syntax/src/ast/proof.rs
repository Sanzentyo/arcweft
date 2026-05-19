use super::{IdRef, TextRange};

/// Top-level `proof @proof.id { ... }` item kept for verifier lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofItem {
    id: IdRef,
    body: String,
    clauses: Vec<ProofClause>,
    range: TextRange,
}

/// Structured clause inside a top-level `proof` item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofClause {
    Requires {
        source: String,
        lifetime_targets: Vec<String>,
    },
    Ensures {
        source: String,
        lifetime_targets: Vec<String>,
    },
    Check {
        source: String,
        lifetime_targets: Vec<String>,
    },
    Assume {
        source: String,
        reason: Option<String>,
        axiom: Option<String>,
    },
    UseAxiom {
        id: String,
    },
    Raw {
        source: String,
    },
}

/// Top-level `trusted axiom @axiom.id { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedAxiomItem {
    id: IdRef,
    body: String,
    range: TextRange,
}

/// Top-level `test @test.id kind { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestItem {
    id: IdRef,
    kind: TestKind,
    body: String,
    range: TextRange,
}

/// Script test category selected after the test id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestKind {
    Scenario,
    Visual,
    Audio,
    Fixture,
    Custom(String),
}

/// Top-level `bench @bench.id { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchItem {
    id: IdRef,
    body: String,
    range: TextRange,
}

impl ProofItem {
    pub(crate) const fn new(
        id: IdRef,
        body: String,
        clauses: Vec<ProofClause>,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            body,
            clauses,
            range,
        }
    }

    pub const fn id(&self) -> &IdRef {
        &self.id
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn clauses(&self) -> &[ProofClause] {
        &self.clauses
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl TrustedAxiomItem {
    pub(crate) const fn new(id: IdRef, body: String, range: TextRange) -> Self {
        Self { id, body, range }
    }

    pub const fn id(&self) -> &IdRef {
        &self.id
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl TestItem {
    pub(crate) const fn new(id: IdRef, kind: TestKind, body: String, range: TextRange) -> Self {
        Self {
            id,
            kind,
            body,
            range,
        }
    }

    pub const fn id(&self) -> &IdRef {
        &self.id
    }

    pub const fn kind(&self) -> &TestKind {
        &self.kind
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl TestKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Scenario => "scenario",
            Self::Visual => "visual",
            Self::Audio => "audio",
            Self::Fixture => "fixture",
            Self::Custom(kind) => kind,
        }
    }
}

impl BenchItem {
    pub(crate) const fn new(id: IdRef, body: String, range: TextRange) -> Self {
        Self { id, body, range }
    }

    pub const fn id(&self) -> &IdRef {
        &self.id
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
