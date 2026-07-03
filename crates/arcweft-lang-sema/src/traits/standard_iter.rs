//! Standard iterator traits installed into the DSL trait catalog.

use super::{
    AssociatedTypeAssignment, AssociatedTypeId, AssociatedTypeRequirement, ImplId, TraitCatalog,
    TraitDecl, TraitId, TraitImpl, TraitMethodImpl, TraitMethodRequirement, TraitWitness,
    TraitWitnessId, trait_type_ref_kind,
};
use crate::types::{IteratorStateKind, TypeKind};
use arcweft_lang_syntax::types::parse_fn_signature;
use std::collections::{BTreeMap, HashSet};

pub(super) const ITERATOR: &str = "Iterator";
pub(super) const INTO_ITERATOR: &str = "IntoIterator";
pub(super) const ITEM: &str = "Item";
pub(super) const INTO_ITER: &str = "IntoIter";

const TYPE_PARAM_T: &str = "T";
const TYPE_PARAM_E: &str = "E";
const TYPE_PARAM_N: &str = "N";

pub(super) fn install_standard_iterator_traits(
    catalog: &mut TraitCatalog,
    next_assoc_id: &mut usize,
) {
    let iterator_id = ensure_trait(catalog, ITERATOR);
    let into_iterator_id = ensure_trait(catalog, INTO_ITERATOR);

    if let Some(trait_decl) = catalog.traits.get_mut(iterator_id.index()) {
        ensure_assoc(trait_decl, next_assoc_id, ITEM);
        ensure_method(trait_decl, "fn next(&mut self) -> Option<Self::Item>");
    }

    if let Some(trait_decl) = catalog.traits.get_mut(into_iterator_id.index()) {
        ensure_assoc(trait_decl, next_assoc_id, ITEM);
        ensure_assoc(trait_decl, next_assoc_id, INTO_ITER);
        ensure_method(trait_decl, "fn into_iter(self) -> Self::IntoIter");
    }
}

pub(super) fn install_standard_iterator_impls(catalog: &mut TraitCatalog) {
    let Some(iterator) = catalog.trait_id(ITERATOR) else {
        return;
    };
    let Some(into_iterator) = catalog.trait_id(INTO_ITERATOR) else {
        return;
    };

    let t = TypeKind::GenericParam(TYPE_PARAM_T.to_owned());
    let e = TypeKind::GenericParam(TYPE_PARAM_E.to_owned());
    let n = TYPE_PARAM_N.to_owned();

    push_builtin_impl(
        catalog,
        iterator,
        iterator_state(IteratorStateKind::Range, t.clone()),
        [(ITEM, t.clone())],
        [method_impl(
            iterator,
            "fn next(&mut self) -> Option<Self::Item>",
        )],
    );
    push_builtin_impl(
        catalog,
        into_iterator,
        TypeKind::Range(Box::new(t.clone())),
        [
            (ITEM, t.clone()),
            (
                INTO_ITER,
                iterator_state(IteratorStateKind::Range, t.clone()),
            ),
        ],
        [method_impl(
            into_iterator,
            "fn into_iter(self) -> Self::IntoIter",
        )],
    );

    push_builtin_impl(
        catalog,
        iterator,
        TypeKind::Seq(Box::new(t.clone())),
        [(ITEM, t.clone())],
        [method_impl(
            iterator,
            "fn next(&mut self) -> Option<Self::Item>",
        )],
    );
    push_builtin_impl(
        catalog,
        into_iterator,
        TypeKind::Seq(Box::new(t.clone())),
        [
            (ITEM, t.clone()),
            (INTO_ITER, iterator_state(IteratorStateKind::Seq, t.clone())),
        ],
        [method_impl(
            into_iterator,
            "fn into_iter(self) -> Self::IntoIter",
        )],
    );

    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Stream,
        TypeKind::Stream {
            item: Box::new(t.clone()),
            error: Box::new(e),
        },
        t.clone(),
    );
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Vec,
        TypeKind::Vec(Box::new(t.clone())),
        t.clone(),
    );
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Array,
        TypeKind::Array {
            item: Box::new(t.clone()),
            len: n,
        },
        t.clone(),
    );
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Slice,
        TypeKind::Slice(Box::new(t.clone())),
        t,
    );
}

fn install_container_into_iter(
    catalog: &mut TraitCatalog,
    iterator: TraitId,
    into_iterator: TraitId,
    family: IteratorStateKind,
    source: TypeKind,
    item: TypeKind,
) {
    let iter_ty = iterator_state(family, item.clone());
    push_builtin_impl(
        catalog,
        iterator,
        iter_ty.clone(),
        [(ITEM, item.clone())],
        [method_impl(
            iterator,
            "fn next(&mut self) -> Option<Self::Item>",
        )],
    );
    push_builtin_impl(
        catalog,
        into_iterator,
        source,
        [(ITEM, item), (INTO_ITER, iter_ty)],
        [method_impl(
            into_iterator,
            "fn into_iter(self) -> Self::IntoIter",
        )],
    );
}

fn ensure_trait(catalog: &mut TraitCatalog, name: &str) -> TraitId {
    if let Some(id) = catalog.trait_id(name) {
        return id;
    }
    let id = TraitId::from_index(catalog.traits.len());
    catalog.by_name.insert(name.to_owned(), id);
    catalog.traits.push(TraitDecl {
        id,
        name: name.to_owned(),
        supertraits: Vec::new(),
        associated_types: Vec::new(),
        methods: Vec::new(),
    });
    id
}

fn ensure_assoc(trait_decl: &mut TraitDecl, next_assoc_id: &mut usize, name: &str) {
    if trait_decl
        .associated_types
        .iter()
        .any(|assoc| assoc.name == name)
    {
        return;
    }
    let id = AssociatedTypeId::from_index(*next_assoc_id);
    *next_assoc_id += 1;
    trait_decl.associated_types.push(AssociatedTypeRequirement {
        id,
        trait_id: trait_decl.id,
        name: name.to_owned(),
    });
}

fn ensure_method(trait_decl: &mut TraitDecl, source: &str) {
    let signature = parse_fn_signature(source)
        .unwrap_or_else(|error| panic!("invalid built-in iterator signature `{source}`: {error}"));
    if trait_decl
        .methods
        .iter()
        .any(|method| method.name == signature.name())
    {
        return;
    }
    trait_decl.methods.push(TraitMethodRequirement {
        trait_id: trait_decl.id,
        name: signature.name().to_owned(),
        signature,
    });
}

fn method_impl(trait_id: TraitId, source: &str) -> TraitMethodImpl {
    let signature = parse_fn_signature(source).unwrap_or_else(|error| {
        panic!("invalid built-in iterator impl signature `{source}`: {error}")
    });
    let return_type = signature.return_type().map_or(TypeKind::Unit, |ty| {
        trait_type_ref_kind(ty, &HashSet::new())
    });
    TraitMethodImpl {
        trait_id: Some(trait_id),
        signature,
        return_type,
        body_is_present: true,
    }
}

fn push_builtin_impl<const A: usize, const M: usize>(
    catalog: &mut TraitCatalog,
    trait_id: TraitId,
    target: TypeKind,
    associated_types: [(&str, TypeKind); A],
    methods: [TraitMethodImpl; M],
) {
    let id = ImplId::from_index(catalog.impls.len());
    let witness = TraitWitnessId::from_index(catalog.witnesses.len());
    let associated_types = associated_types
        .into_iter()
        .map(|(name, value)| {
            (
                name.to_owned(),
                AssociatedTypeAssignment {
                    name: name.to_owned(),
                    value,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let methods = methods
        .into_iter()
        .map(|method| (method.signature().name().to_owned(), method))
        .collect::<BTreeMap<_, _>>();
    catalog.witnesses.push(TraitWitness {
        id: witness,
        impl_id: id,
        trait_id,
        self_ty: target.clone(),
    });
    catalog.exact_impls.insert((trait_id, target.clone()), id);
    catalog.impls.push(TraitImpl {
        id,
        trait_id: Some(trait_id),
        target,
        associated_types,
        methods,
        witness: Some(witness),
    });
}

fn iterator_state(family: IteratorStateKind, item: TypeKind) -> TypeKind {
    TypeKind::IteratorState {
        family,
        item: Box::new(item),
    }
}
