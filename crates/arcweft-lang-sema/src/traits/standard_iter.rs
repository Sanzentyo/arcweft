//! Standard iterator traits installed into the DSL trait catalog.

use super::{
    AssociatedTypeAssignment, AssociatedTypeId, AssociatedTypeRequirement, ImplId, TraitCatalog,
    TraitDecl, TraitId, TraitImpl, TraitMethodImpl, TraitMethodRequirement, TraitWitness,
    TraitWitnessId, trait_method_param_groups,
};
use crate::env::TypeCheckEnv;
use crate::nominal::{
    GenericTypeScope, NominalResolutionLimits, SelfTypeScope, TypeResolutionInput, resolve_type_ref,
};
use crate::types::{
    ArrayLength, DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId,
    IteratorStateKind, TypeKind,
};
use arcweft_lang_syntax::types::parse_fn_signature;
use std::collections::BTreeMap;

pub(super) const ITERATOR: &str = "Iterator";
pub(super) const INTO_ITERATOR: &str = "IntoIterator";
pub(super) const ITEM: &str = "Item";
pub(super) const INTO_ITER: &str = "IntoIter";

const STANDARD_ITERATOR_GENERIC_OWNER_BASE: u64 = 0x4157_4954_4552_0000;

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

    install_range_iterator_impls(catalog, iterator, into_iterator);
    install_sequence_iterator_impls(catalog, iterator, into_iterator);

    let t = standard_generic_parameter(3, 0);
    let e = standard_generic_parameter(3, 1);
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Stream,
        &TypeKind::Stream {
            item: Box::new(t.clone()),
            error: Box::new(e),
        },
        t.clone(),
    );
    let t = standard_generic_parameter(4, 0);
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Vec,
        &TypeKind::Vec(Box::new(t.clone())),
        t.clone(),
    );
    let t = standard_generic_parameter(5, 0);
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Array,
        &TypeKind::Array {
            item: Box::new(t.clone()),
            len: ArrayLength::Generic(standard_generic_parameter_id(5, 1)),
        },
        t.clone(),
    );
    let t = standard_generic_parameter(6, 0);
    install_container_into_iter(
        catalog,
        iterator,
        into_iterator,
        IteratorStateKind::Slice,
        &TypeKind::Slice(Box::new(t.clone())),
        t,
    );
}

fn install_range_iterator_impls(
    catalog: &mut TraitCatalog,
    iterator: TraitId,
    into_iterator: TraitId,
) {
    let t = standard_generic_parameter(1, 0);
    let range_iterator = iterator_state(IteratorStateKind::Range, t.clone());
    push_builtin_impl(
        catalog,
        iterator,
        range_iterator.clone(),
        [(ITEM, t.clone())],
        [method_impl(
            iterator,
            "fn next(&mut self) -> Option<Self::Item>",
            &range_iterator,
        )],
    );
    let range = TypeKind::Range(Box::new(t.clone()));
    push_builtin_impl(
        catalog,
        into_iterator,
        range.clone(),
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
            &range,
        )],
    );
}

fn install_sequence_iterator_impls(
    catalog: &mut TraitCatalog,
    iterator: TraitId,
    into_iterator: TraitId,
) {
    let t = standard_generic_parameter(2, 0);
    let seq = TypeKind::Seq(Box::new(t.clone()));
    push_builtin_impl(
        catalog,
        iterator,
        seq.clone(),
        [(ITEM, t.clone())],
        [method_impl(
            iterator,
            "fn next(&mut self) -> Option<Self::Item>",
            &seq,
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
            &seq,
        )],
    );
}

fn standard_generic_parameter(owner_ordinal: u64, parameter_ordinal: u16) -> TypeKind {
    TypeKind::GenericParam(standard_generic_parameter_id(
        owner_ordinal,
        parameter_ordinal,
    ))
}

fn standard_generic_parameter_id(
    owner_ordinal: u64,
    parameter_ordinal: u16,
) -> GenericTypeParameterId {
    let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(
        STANDARD_ITERATOR_GENERIC_OWNER_BASE + owner_ordinal,
    ));
    GenericTypeParameterId::new(owner, parameter_ordinal)
}

fn install_container_into_iter(
    catalog: &mut TraitCatalog,
    iterator: TraitId,
    into_iterator: TraitId,
    family: IteratorStateKind,
    source: &TypeKind,
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
            &iter_ty,
        )],
    );
    push_builtin_impl(
        catalog,
        into_iterator,
        source.clone(),
        [(ITEM, item), (INTO_ITER, iter_ty)],
        [method_impl(
            into_iterator,
            "fn into_iter(self) -> Self::IntoIter",
            source,
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
    let self_parameter = standard_generic_parameter_id(
        100 + u64::try_from(trait_decl.id.index()).expect("trait id fits u64"),
        0,
    );
    let self_scope = SelfTypeScope::Known(TypeKind::GenericParam(self_parameter.clone()));
    let generic_scope = GenericTypeScope::empty();
    let environment = TypeCheckEnv::standard();
    let resolve = |authored: &arcweft_lang_syntax::types::AuthoredTypeRef| {
        resolve_type_ref(&TypeResolutionInput::detached(
            authored,
            None,
            &environment,
            &generic_scope,
            self_scope.clone(),
            NominalResolutionLimits::PRODUCTION,
        ))
        .expect("compiled detached nominal limits are valid")
        .outcome()
        .product()
        .recovered()
        .clone()
    };
    let param_groups = trait_method_param_groups(&signature, |ty| Some(resolve(ty)))
        .expect("built-in iterator parameters use accepted detached semantic types");
    let return_type = signature.return_type().map_or(TypeKind::Unit, resolve);
    trait_decl.methods.push(TraitMethodRequirement {
        trait_id: trait_decl.id,
        name: signature.name().to_owned(),
        signature,
        self_parameter,
        param_groups,
        return_type,
    });
}

fn method_impl(trait_id: TraitId, source: &str, self_ty: &TypeKind) -> TraitMethodImpl {
    let signature = parse_fn_signature(source).unwrap_or_else(|error| {
        panic!("invalid built-in iterator impl signature `{source}`: {error}")
    });
    let generic_scope = GenericTypeScope::empty();
    let self_scope = SelfTypeScope::Known(self_ty.clone());
    let environment = TypeCheckEnv::standard();
    let resolve = |authored: &arcweft_lang_syntax::types::AuthoredTypeRef| {
        resolve_type_ref(&TypeResolutionInput::detached(
            authored,
            None,
            &environment,
            &generic_scope,
            self_scope.clone(),
            NominalResolutionLimits::PRODUCTION,
        ))
        .expect("compiled detached nominal limits are valid")
        .outcome()
        .product()
        .recovered()
        .clone()
    };
    let param_groups = trait_method_param_groups(&signature, |ty| Some(resolve(ty)))
        .expect("built-in iterator parameters use accepted detached semantic types");
    let return_type = signature.return_type().map_or(TypeKind::Unit, resolve);
    TraitMethodImpl {
        trait_id: Some(trait_id),
        signature,
        param_groups,
        return_type,
        body: None,
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
