use arcweft_lang_syntax::ast::items::{ImplMember, Item, TraitMember};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_lang_syntax::types::{AuthoredTypeRef, TypeRef};

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
        function
            .signature()
            .return_type()
            .map(AuthoredTypeRef::value),
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
    assert!(matches!(
        item.trait_ref().map(AuthoredTypeRef::value),
        Some(TypeRef::Path(path)) if path.canonical_string() == "SourceLike"
    ));
    assert_eq!(item.where_clauses().len(), 1);
    assert!(matches!(
        item.members()[0],
        ImplMember::AssociatedType { .. }
    ));
}

#[test]
fn trait_and_impl_members_preserve_curried_param_groups() {
    let parsed = parse_source(
        r"
trait Threshold {
    fn above(self, min: i64)(value: i64) -> bool
}

impl Threshold for Score {
    fn above(self, min: i64)(value: i64) -> bool {
        value >= min
    }
}
",
    )
    .into_typed_tree();
    let Item::Trait(trait_item) = &parsed.items()[0] else {
        panic!("trait item expected")
    };
    let TraitMember::Function {
        signature: trait_signature,
        ..
    } = &trait_item.members()[0]
    else {
        panic!("trait function member expected")
    };
    assert_eq!(trait_signature.name(), "above");
    assert_eq!(trait_signature.param_groups().len(), 2);
    assert_eq!(trait_signature.param_groups()[0].params().len(), 2);
    assert_eq!(trait_signature.param_groups()[1].params().len(), 1);

    let Item::Impl(impl_item) = &parsed.items()[1] else {
        panic!("impl item expected")
    };
    let ImplMember::Function {
        signature: impl_signature,
        ..
    } = &impl_item.members()[0]
    else {
        panic!("impl function member expected")
    };
    assert_eq!(impl_signature.name(), "above");
    assert_eq!(impl_signature.param_groups().len(), 2);
    assert_eq!(impl_signature.param_groups()[0].params().len(), 2);
    assert_eq!(impl_signature.param_groups()[1].params().len(), 1);
}
