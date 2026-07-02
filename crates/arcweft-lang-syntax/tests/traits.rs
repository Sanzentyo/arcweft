use arcweft_lang_syntax::ast::items::{ImplMember, Item, TraitMember};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_lang_syntax::types::TypeRef;

#[test]
fn trait_item_preserves_associated_type_and_method_requirement() {
    let parsed = parse_source(
        r"
trait SourceLike {
    type Item
    fn current(self) -> Self::Item
}
",
    )
    .into_typed_tree();
    let Item::Trait(item) = &parsed.items()[0] else {
        panic!("trait item expected")
    };
    assert_eq!(item.name(), "SourceLike");
    assert!(matches!(
        item.members()[0],
        TraitMember::AssociatedType { .. }
    ));
    assert!(matches!(item.members()[1], TraitMember::Function { .. }));
}

#[test]
fn parses_projection_and_assoc_equality_bound() {
    let parsed = parse_source(
        r"
fn exact<T>(source: T) -> T::Item
where T: SourceLike<Item = ChapterId>
{
    source.current()
}
",
    )
    .into_typed_tree();
    let Item::Function(function) = &parsed.items()[0] else {
        panic!("function item expected")
    };
    assert!(matches!(
        function.signature().return_type(),
        Some(TypeRef::Projection { .. })
    ));
    assert_eq!(function.signature().where_clauses().len(), 1);
}

#[test]
fn impl_item_preserves_where_clause_and_associated_assignment() {
    let parsed = parse_source(
        r"
impl<T> SourceLike for Box<T>
where T: Copyable
{
    type Item = T
    fn current(self) -> T { self.value }
}
",
    )
    .into_typed_tree();
    let Item::Impl(item) = &parsed.items()[0] else {
        panic!("impl item expected")
    };
    assert_eq!(item.trait_name(), Some("SourceLike"));
    assert_eq!(item.where_clauses().len(), 1);
    assert!(matches!(
        item.members()[0],
        ImplMember::AssociatedType { .. }
    ));
}
