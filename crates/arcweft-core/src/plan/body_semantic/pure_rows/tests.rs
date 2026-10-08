use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableParameterSeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeLocalDeclarationSeed, RuntimePlanBuilder,
    RuntimePlanInventory, RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureHelperOrigin,
    RuntimePureHelperSeed, RuntimeTaskPlanSealLimits, RuntimeTraitMethodIdentity,
    RuntimeTraitMethodSeed,
};
use crate::task::semantic::TaskSemanticEncodingError;
use crate::value::RuntimeValue;

enum RowKind {
    Helper,
    Method,
}

struct Fixture {
    kind: RowKind,
    padding: bool,
    value: bool,
    definition: u8,
    output: RuntimePureOutputType,
    reverse: bool,
    diagnostic: usize,
    receiver: RuntimeReceiverMode,
}

impl Fixture {
    fn new(method: bool) -> Self {
        Self {
            kind: if method {
                RowKind::Method
            } else {
                RowKind::Helper
            },
            padding: false,
            value: true,
            definition: 21,
            output: RuntimePureOutputType::Bool,
            reverse: false,
            diagnostic: 0,
            receiver: RuntimeReceiverMode::Owned,
        }
    }

    fn inventory(&self) -> RuntimePlanInventory {
        let mut builder = RuntimePlanBuilder::new();
        let boolean = RuntimeSemanticTypeId::from_bytes([1; 32]);
        let batch = builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    boolean,
                    RuntimePlanTypeProjection::Bool,
                )],
                [
                    RuntimeLocalDeclarationSeed::new(
                        fixture_binding_source([11; 32], false),
                        boolean,
                    ),
                    RuntimeLocalDeclarationSeed::new(
                        fixture_binding_source([12; 32], false),
                        boolean,
                    ),
                ],
            )
            .unwrap();
        let mut inputs = batch
            .local_ids()
            .iter()
            .enumerate()
            .map(|(ordinal, local)| RuntimeCallableParameterSeed {
                identity: RuntimeFunctionParameterIdentity::from_accepted_identity(
                    [31 + u8::try_from(ordinal).unwrap(); 32],
                ),
                local: local.clone(),
                passing: RuntimeFunctionParameterPassing::Value,
                abi: RuntimePureInputType::Value,
            })
            .collect::<Vec<_>>();
        if self.reverse {
            inputs.reverse();
        }
        let body = RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(RuntimeValue::Bool(self.value)),
        );
        if matches!(self.kind, RowKind::Method) {
            let identity = RuntimeTraitMethodIdentity {
                impl_id: self.diagnostic,
                trait_id: Some(self.diagnostic + 1),
                witness: Some(self.diagnostic + 2),
                trait_name: Some(format!("trait{}", self.diagnostic)),
                self_type: format!("self{}", self.diagnostic),
                method_name: format!("target{}", self.diagnostic),
                monomorph_label: format!("mono{}", self.diagnostic),
            };
            let seed = RuntimeTraitMethodSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [self.definition; 32],
                ),
                identity,
                receiver: self.receiver,
                inputs: inputs.into_boxed_slice(),
                output_abi: self.output,
                body,
            };
            if self.padding {
                let mut padding = seed.clone();
                padding.definition =
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]);
                padding.identity.method_name = "padding".into();
                builder.push_trait_method_seed(padding).unwrap();
            }
            builder.push_trait_method_seed(seed).unwrap();
        } else {
            let seed = RuntimePureHelperSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [self.definition; 32],
                ),
                name: format!("helper{}", self.diagnostic),
                inputs: inputs.into_boxed_slice(),
                output_abi: self.output,
                body,
                scalar_eval_supported: self.diagnostic == 0,
                origin: if self.diagnostic == 0 {
                    RuntimePureHelperOrigin::Annotated
                } else {
                    RuntimePureHelperOrigin::Inferred
                },
            };
            if self.padding {
                let mut padding = seed.clone();
                padding.definition =
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]);
                padding.name = "padding".into();
                padding.inputs = Box::new([]);
                builder.push_pure_helper_seed(padding).unwrap();
            }
            builder.push_pure_helper_seed(seed).unwrap();
        }
        let inventory = builder
            .prepare_inventory(RuntimeTaskPlanSealLimits::default())
            .unwrap();
        inventory.verify().unwrap();
        inventory
    }
}

fn digest(
    inventory: &RuntimePlanInventory,
    method: bool,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let context = RuntimeBodySemanticContext::new(inventory);
    let result = if method {
        inventory
            .trait_methods()
            .last()
            .unwrap()
            .executable_semantic_row_digest(&context, &mut meter)
    } else {
        inventory
            .pure_helpers()
            .last()
            .unwrap()
            .executable_semantic(&context, &mut meter)
            .map(|semantic| semantic.digest())
    };
    (result, meter.totals())
}

#[test]
fn admitted_pure_rows_exclude_diagnostics_backend_choice_and_arena_padding() {
    for method in [false, true] {
        let first = Fixture::new(method).inventory();
        let mut changed = Fixture::new(method);
        changed.padding = true;
        changed.diagnostic = 700;
        assert_eq!(
            digest(&first, method, 10_000, 100_000).0.unwrap(),
            digest(&changed.inventory(), method, 10_000, 100_000)
                .0
                .unwrap()
        );
    }
}

#[test]
fn admitted_definition_input_order_result_abi_and_typed_body_change_the_row() {
    for method in [false, true] {
        let baseline = Fixture::new(method);
        let expected = digest(&baseline.inventory(), method, 10_000, 100_000)
            .0
            .unwrap();
        for role in 0..4 {
            let mut changed = Fixture::new(method);
            match role {
                0 => changed.definition += 1,
                1 => changed.reverse = true,
                2 => changed.output = RuntimePureOutputType::Value,
                3 => changed.value = false,
                _ => unreachable!(),
            }
            assert_ne!(
                expected,
                digest(&changed.inventory(), method, 10_000, 100_000)
                    .0
                    .unwrap()
            );
        }
    }
}

#[test]
fn admitted_trait_receiver_modes_have_distinct_rows() {
    let mut hashes = std::collections::BTreeSet::new();
    for receiver in [
        RuntimeReceiverMode::Owned,
        RuntimeReceiverMode::SharedRef,
        RuntimeReceiverMode::MutRef,
    ] {
        let mut fixture = Fixture::new(true);
        fixture.receiver = receiver;
        assert!(
            hashes.insert(
                *digest(&fixture.inventory(), true, 10_000, 100_000)
                    .0
                    .unwrap()
                    .as_bytes()
            )
        );
    }
}

#[test]
fn pure_rows_share_exact_limits_and_reject_foreign_owner_or_inherited_poison() {
    for method in [false, true] {
        let inventory = Fixture::new(method).inventory();
        let (expected, (work, bytes)) = digest(&inventory, method, 10_000, 100_000);
        assert_eq!(
            digest(&inventory, method, work, bytes).0.unwrap(),
            expected.unwrap()
        );
        assert!(matches!(
            digest(&inventory, method, work - 1, bytes).0,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert!(matches!(
            digest(&inventory, method, work, bytes - 1).0,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::TranscriptBytes
            ))
        ));
        let foreign = inventory.clone();
        let context = RuntimeBodySemanticContext::new(&foreign);
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        let result = if method {
            inventory
                .trait_methods()
                .last()
                .unwrap()
                .executable_semantic_row_digest(&context, &mut meter)
        } else {
            inventory
                .pure_helpers()
                .last()
                .unwrap()
                .executable_semantic(&context, &mut meter)
                .map(|semantic| semantic.digest())
        };
        assert!(matches!(
            result,
            Err(RuntimeBodySemanticError::ForeignTraitMethodRow
                | RuntimeBodySemanticError::ForeignPureHelperRow)
        ));
        assert_eq!(meter.totals(), (0, 0));
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
        let mut poisoned = TaskSemanticMeter::new(0, 100_000);
        poisoned.charge_work(1).unwrap_err();
        let before = poisoned.totals();
        let context = RuntimeBodySemanticContext::new(&inventory);
        let result = if method {
            inventory
                .trait_methods()
                .last()
                .unwrap()
                .executable_semantic_row_digest(&context, &mut poisoned)
        } else {
            inventory
                .pure_helpers()
                .last()
                .unwrap()
                .executable_semantic(&context, &mut poisoned)
                .map(|semantic| semantic.digest())
        };
        assert!(matches!(
            result,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert_eq!(poisoned.totals(), before);
    }
}

fn fixture_binding_source(
    identity: [u8; 32],
    mutable: bool,
) -> crate::plan::RuntimeLocalDeclarationSource {
    crate::plan::RuntimeLocalDeclarationSource::Binding {
        identity,
        declaration: crate::plan::RuntimeLocalBindingDeclaration::new(
            crate::plan::RuntimeLocalBindingKind::PatternBinding,
            mutable,
            crate::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}
