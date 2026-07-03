//! Compiler bridge from sema witnesses to runtime-plan trait method callables.

use arcweft_core::plan::{
    RuntimeIteratorWitnessCalls, RuntimeIteratorWitnessEvidence, RuntimeIteratorWitnessExecutable,
    RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode, RuntimeTraitMethodIdentity,
};
use arcweft_lang_sema::check::{ForIterationEvidence, ForIterationEvidenceFamily};
use arcweft_lang_sema::traits::{TraitCatalog, TraitMethodForWitness, TraitWitnessId};
use arcweft_lang_syntax::ast::pattern::Pattern;
use arcweft_lang_syntax::types::{FnParam, FnSignature};
use arcweft_runtime_plan::trait_methods::{
    RuntimeTraitMethodInventory, TraitMethodLowerInput, lower_trait_method_inventory,
};
use std::collections::BTreeSet;

pub fn lower_runtime_trait_methods_from_typecheck(
    catalog: &TraitCatalog,
    evidence: &[ForIterationEvidence],
) -> Result<RuntimeTraitMethodInventory, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    let mut required = BTreeSet::new();
    for item in evidence {
        if let ForIterationEvidenceFamily::Witness {
            into_iterator,
            iterator,
        } = item.family
        {
            required.insert((into_iterator, "into_iter"));
            required.insert((iterator, "next"));
        }
    }
    let inputs = required
        .into_iter()
        .filter_map(|(witness, method)| trait_method_input(catalog, witness, method))
        .collect::<Vec<_>>();
    lower_trait_method_inventory(inputs)
}

fn trait_method_input<'a>(
    catalog: &'a TraitCatalog,
    witness: TraitWitnessId,
    method_name: &'static str,
) -> Option<TraitMethodLowerInput<'a>> {
    let method = catalog.witness_method(witness, method_name)?;
    let body = method.body()?;
    let identity = RuntimeTraitMethodIdentity {
        impl_id: method.impl_id().index(),
        trait_id: Some(method.trait_id().index()),
        witness: Some(method.witness().index()),
        trait_name: catalog.trait_name(method.trait_id()).map(str::to_owned),
        self_type: format!("{:?}", method.self_ty()),
        method_name: method_name.to_owned(),
        monomorph_label: format!("witness#{}::{method_name}", witness.index()),
    };
    let input_names = method_input_names(&method);
    let input_types = vec![RuntimePureInputType::Value; input_names.len()];
    Some(TraitMethodLowerInput {
        identity,
        receiver: receiver_mode(method.signature())?,
        input_names,
        input_types,
        output_type: RuntimePureOutputType::Value,
        statements: body.statements(),
        value: body.value(),
    })
}

fn receiver_mode(signature: &FnSignature) -> Option<RuntimeReceiverMode> {
    let receiver = signature_params(signature).next()?;
    let name = param_name(receiver)?;
    if name == "self" {
        let ty = format!("{:?}", receiver.ty());
        if ty.contains("&mut") {
            Some(RuntimeReceiverMode::MutRef)
        } else if ty.contains('&') {
            Some(RuntimeReceiverMode::SharedRef)
        } else {
            Some(RuntimeReceiverMode::Owned)
        }
    } else {
        None
    }
}

fn method_input_names(method: &TraitMethodForWitness<'_>) -> Vec<String> {
    signature_params(method.signature())
        .filter_map(param_name)
        .map(str::to_owned)
        .collect()
}

fn signature_params(signature: &FnSignature) -> impl Iterator<Item = &FnParam> {
    signature
        .param_groups()
        .iter()
        .flat_map(|group| group.params().iter())
}

fn param_name(param: &FnParam) -> Option<&str> {
    match param.pattern() {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => Some(name),
        _ => None,
    }
}

pub fn runtime_witness_evidence(
    item_type: String,
    into_iter_type: String,
    inventory: &RuntimeTraitMethodInventory,
    into_iterator: TraitWitnessId,
    iterator: TraitWitnessId,
) -> Option<RuntimeIteratorWitnessEvidence> {
    let into_iter = inventory
        .by_witness_method
        .get(&(into_iterator.index(), "into_iter".to_owned()))
        .copied()?;
    let next = inventory
        .by_witness_method
        .get(&(iterator.index(), "next".to_owned()))
        .copied()?;
    Some(RuntimeIteratorWitnessEvidence {
        item_type,
        into_iter_type,
        executable: RuntimeIteratorWitnessExecutable::TraitCalls(RuntimeIteratorWitnessCalls {
            into_iter,
            next,
        }),
    })
}
