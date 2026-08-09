//! Typed source-role vocabulary for final HIR item owners.

use super::flow_role::HirFlowSourceRole;
use super::style_role::HirStyleSourceRole;
use super::view_role::HirViewSourceRole;

/// Source component shared by source-backed declaration families.
///
/// `Whole` is retained by the item's immutable slot and applies to ordinary
/// named declarations as well as retained public-ID declarations. `Name`, the
/// Proof-trust roles, and the nominal-member roles are the exact parser-owned
/// components of `Function`, `Predicate`, `Proof`, `Struct`, `Enum`, and
/// `TypeAlias` items.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationSourceRole {
    Whole,
    Name,
    /// Complete accepted `verify.trusted(...)` attribute on a Proof.
    ProofTrustAttribute,
    /// Exact authored string expression carrying the trusted Proof reason.
    ProofTrustReason,
    StructField {
        field: u32,
        part: HirNominalMemberSourcePart,
    },
    EnumVariant {
        variant: u32,
        part: HirNominalMemberSourcePart,
    },
}

/// Exact source component of one ordered Struct field or Enum variant.
///
/// Payload types retain their own `TypeId` source owner. These roles retain
/// only the nominal member container and required name so project symbols do
/// not fabricate either span from the payload type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirNominalMemberSourcePart {
    Whole,
    Name,
}

/// Exact source component owned by one final Entry declaration.
///
/// The whole declaration resolves through the immutable item slot. `Id`
/// resolves the authored entity-reference expression or its checked missing
/// expression insertion site. Entry kind is semantic data on `HirEntryKind`
/// and is deliberately not a second source component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntrySourcePart {
    Whole,
    Id,
    /// Exact right-hand-side source of one typed Entry role member.
    ///
    /// The ordinal is the member's retained source-order coordinate. Route and
    /// option payloads keep their own final expression owners and are not
    /// admitted through this role.
    MemberValue {
        member: u32,
    },
}

/// Final-HIR callable whose source components are owned by one item query.
///
/// Ordinary Function, Predicate, and Proof declarations use `Item`. View uses
/// the distinct `ViewItem` owner because its callable parameter surface does
/// not make it an ordinary runtime-callable declaration. External-capability,
/// Trait, and Impl functions remain inline members qualified by their checked
/// source ordinal; no detached member-source table exists.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallableSourceOwner {
    Item,
    ViewItem,
    ExternCapabilityFunction { member: u16 },
    TraitFunction { member: u16 },
    ImplFunction { member: u16 },
}

/// Source component of one authored callable parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallableParameterSourcePart {
    Whole,
    Name,
    Type,
    Default,
}

/// Exact source component of one authored callable effect clause.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallableEffectSourcePart {
    /// Complete `effects { ... }` clause.
    Whole,
    /// Exact authored `effects` keyword token.
    Keyword,
}

/// Typed callable component retained by the sole final-HIR source index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallableSourceRole {
    Name {
        owner: HirCallableSourceOwner,
    },
    Signature {
        owner: HirCallableSourceOwner,
    },
    Result {
        owner: HirCallableSourceOwner,
    },
    Parameter {
        owner: HirCallableSourceOwner,
        group: u16,
        parameter: u16,
        part: HirCallableParameterSourcePart,
    },
    /// One source-ordered authored effect clause.
    EffectClause {
        owner: HirCallableSourceOwner,
        clause: u16,
        part: HirCallableEffectSourcePart,
    },
}

impl HirCallableSourceRole {
    pub const fn owner(self) -> HirCallableSourceOwner {
        match self {
            Self::Name { owner }
            | Self::Signature { owner }
            | Self::Result { owner }
            | Self::Parameter { owner, .. }
            | Self::EffectClause { owner, .. } => owner,
        }
    }
}

/// Exact source component of one flattened semantic use binding.
///
/// `Path` selects the parser-owned direct path or grouped-import module path.
/// `TerminalReference` selects the final imported name, or the authored `*`
/// for a glob. `Alias` selects the complete parser-owned alias clause and is
/// optional for every binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUseBindingSourcePart {
    Path,
    TerminalReference,
    Alias,
}

/// Typed source role owned by one final `HirUseDeclaration`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUseSourceRole {
    Whole,
    Binding {
        ordinal: u32,
        part: HirUseBindingSourcePart,
    },
}

/// Exact source component shared by script Test and Bench declarations.
///
/// The complete declaration remains on the immutable item slot. Test and
/// Bench intentionally share this role because both are statement-only plan
/// owners and neither admits a second detached syntax payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTestBenchSourceRole {
    Whole,
}

/// Typed item source-role family admitted by the sole source index.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemSourceRole {
    Declaration(HirDeclarationSourceRole),
    Entry(HirEntrySourcePart),
    Callable(HirCallableSourceRole),
    Use(HirUseSourceRole),
    TestBench(HirTestBenchSourceRole),
    Flow(HirFlowSourceRole),
    Style(HirStyleSourceRole),
    View(HirViewSourceRole),
}
