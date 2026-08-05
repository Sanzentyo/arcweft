use super::common::TextRange;
use super::ids::IdRef;
use super::items::Attribute;

/// Top-level `proof name { ... }` item kept for verifier lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofItem {
    id: IdRef,
    name: String,
    explicit_id: bool,
    attrs: Vec<Attribute>,
    trust: ProofTrust,
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
        proof: Option<String>,
    },
    UseProof {
        id: String,
    },
    Raw {
        source: String,
    },
}

/// Typed trust status attached to one proof declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofTrust {
    Verified,
    Trusted {
        reason: String,
        attribute_range: TextRange,
    },
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
        name: String,
        explicit_id: bool,
        attrs: Vec<Attribute>,
        trust: ProofTrust,
        body: String,
        clauses: Vec<ProofClause>,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            name,
            explicit_id,
            attrs,
            trust,
            body,
            clauses,
            range,
        }
    }

    pub const fn id(&self) -> &IdRef {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn has_explicit_id(&self) -> bool {
        self.explicit_id
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn trust(&self) -> &ProofTrust {
        &self.trust
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
