//! Exhaustive attached source-item classification.
//!
//! Every item handle is one exact parser-owned grammar kind. Consumers do not
//! rediscover declaration families from keywords, source text, or diagnostics.

use arcweft_source::{SourceRange, SourceSpan};

use super::node::{
    ActionDeclarationItemKind, ActivityDeclarationItemKind, AstNode, BenchItemKind,
    CharacterDeclarationItemKind, EntryDeclarationItemKind, EnumItemKind, ErrorItemKind,
    ExternCapabilityItemKind, FlowItemKind, FunctionItemKind, ImplItemKind,
    LayerDeclarationItemKind, MetricDeclarationItemKind, ModuleDeclarationKind, PredicateItemKind,
    ProofItemKind, ResourceDeclarationItemKind, SignalDeclarationItemKind, StructItemKind,
    StyleItemKind, TestItemKind, TraitItemKind, TypeAliasItemKind, UseDeclarationKind,
    ViewDeclarationItemKind,
};
use super::{SyntaxLookupError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId};
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};

macro_rules! typed_item_inventory {
    ($($variant:ident($marker:ident) => $kind:ident),+ $(,)?) => {
        /// Exact attached source-item inventory in grammar order.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum TypedItemNode {
            $($variant(AstNode<$marker>)),+
        }

        impl TypedItemNode {
            pub(crate) fn from_syntax(node: SyntaxNodeHandle) -> Result<Self, SyntaxLookupError> {
                match node.kind() {
                    $(SyntaxKind::$kind => Ok(Self::$variant(node.cast()?))),+,
                    actual => Err(SyntaxLookupError::KindPredicateMismatch {
                        id: node.id(),
                        expected: AstTag::Item,
                        actual,
                    }),
                }
            }

            /// Returns the exact parser-owned item kind.
            pub const fn kind(&self) -> SyntaxKind {
                match self {
                    $(Self::$variant(_) => SyntaxKind::$kind),+
                }
            }

            /// Returns the immutable syntax identity of this item.
            pub fn id(&self) -> SyntaxNodeId {
                match self {
                    $(Self::$variant(node) => node.id()),+
                }
            }

            /// Returns the immutable syntax snapshot owning this item.
            pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
                match self {
                    $(Self::$variant(node) => node.snapshot_id()),+
                }
            }

            /// Returns the exact typed syntax handle without family erasure.
            pub fn syntax(&self) -> SyntaxNodeHandle {
                match self {
                    $(Self::$variant(node) => node.syntax()),+
                }
            }

            /// Returns the exact source range owned by this item.
            pub fn range(&self) -> SourceRange {
                match self {
                    $(Self::$variant(node) => node.range()),+
                }
            }

            /// Returns the exact source revision and range owned by this item.
            pub fn source_span(&self) -> SourceSpan {
                match self {
                    $(Self::$variant(node) => node.source_span()),+
                }
            }

            /// Returns the source-file role assigned by the parser.
            pub fn role(&self) -> SyntaxRole {
                match self {
                    $(Self::$variant(node) => node.role()),+
                }
            }
        }
    };
}

typed_item_inventory!(
    Module(ModuleDeclarationKind) => ModuleDeclaration,
    Use(UseDeclarationKind) => UseDeclaration,
    Flow(FlowItemKind) => FlowItem,
    Function(FunctionItemKind) => FunctionItem,
    Predicate(PredicateItemKind) => PredicateItem,
    Proof(ProofItemKind) => ProofItem,
    Trait(TraitItemKind) => TraitItem,
    Impl(ImplItemKind) => ImplItem,
    Enum(EnumItemKind) => EnumItem,
    Struct(StructItemKind) => StructItem,
    TypeAlias(TypeAliasItemKind) => TypeAliasItem,
    Resource(ResourceDeclarationItemKind) => ResourceDeclarationItem,
    Character(CharacterDeclarationItemKind) => CharacterDeclarationItem,
    View(ViewDeclarationItemKind) => ViewDeclarationItem,
    Action(ActionDeclarationItemKind) => ActionDeclarationItem,
    Activity(ActivityDeclarationItemKind) => ActivityDeclarationItem,
    Signal(SignalDeclarationItemKind) => SignalDeclarationItem,
    Metric(MetricDeclarationItemKind) => MetricDeclarationItem,
    Layer(LayerDeclarationItemKind) => LayerDeclarationItem,
    Entry(EntryDeclarationItemKind) => EntryDeclarationItem,
    ExternCapability(ExternCapabilityItemKind) => ExternCapabilityItem,
    Test(TestItemKind) => TestItem,
    Bench(BenchItemKind) => BenchItem,
    Style(StyleItemKind) => StyleItem,
    Error(ErrorItemKind) => ErrorItem,
);

#[cfg(test)]
mod tests {
    use super::TypedItemNode;
    use crate::grammar::kinds::SyntaxKind;

    #[test]
    fn typed_item_inventory_covers_every_item_kind_exactly() {
        let expected = SyntaxKind::ALL
            .iter()
            .copied()
            .filter(|kind| kind.is_item())
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 25);
        assert_eq!(
            expected,
            [
                SyntaxKind::ModuleDeclaration,
                SyntaxKind::UseDeclaration,
                SyntaxKind::FlowItem,
                SyntaxKind::FunctionItem,
                SyntaxKind::PredicateItem,
                SyntaxKind::ProofItem,
                SyntaxKind::TraitItem,
                SyntaxKind::ImplItem,
                SyntaxKind::EnumItem,
                SyntaxKind::StructItem,
                SyntaxKind::TypeAliasItem,
                SyntaxKind::ResourceDeclarationItem,
                SyntaxKind::CharacterDeclarationItem,
                SyntaxKind::ViewDeclarationItem,
                SyntaxKind::ActionDeclarationItem,
                SyntaxKind::ActivityDeclarationItem,
                SyntaxKind::SignalDeclarationItem,
                SyntaxKind::MetricDeclarationItem,
                SyntaxKind::LayerDeclarationItem,
                SyntaxKind::EntryDeclarationItem,
                SyntaxKind::ExternCapabilityItem,
                SyntaxKind::TestItem,
                SyntaxKind::BenchItem,
                SyntaxKind::StyleItem,
                SyntaxKind::ErrorItem,
            ]
        );
        let _ = core::mem::size_of::<TypedItemNode>();
    }
}
