# Rust-shaped implementation surfaces

These declarations are normative API shapes, not source files.

## 1. HIR reachability errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirRuntimeReachabilityError {
    #[error("runtime reachability root references an unknown executable owner")]
    UnknownRoot { owner: HirRuntimeExecutableOwner },
    #[error("runtime reachability edge source is unresolved")]
    UnknownEdgeSource { source: HirRuntimeReachabilitySite },
    #[error("runtime reachability edge target is unresolved")]
    UnknownEdgeTarget { target: HirRuntimeExecutableOwner },
    #[error("runtime reachability edge targets a presentation product")]
    PresentationTarget { target: HirRuntimeExecutableOwner },
    #[error("runtime reachability contains a duplicate root")]
    DuplicateRoot { root: HirRuntimeReachabilityRoot },
    #[error("runtime reachability contains a duplicate edge")]
    DuplicateEdge { edge: HirRuntimeReachabilityEdge },
    #[error("runtime reachability contains conflicting edges for one checked source")]
    ConflictingEdge {
        source: HirRuntimeReachabilitySite,
        first: HirRuntimeExecutableOwner,
        second: HirRuntimeExecutableOwner,
    },
    #[error("runtime reachability source does not match its edge kind")]
    InvalidEdgeKind {
        source: HirRuntimeReachabilitySite,
        kind: HirRuntimeReachabilityEdgeKind,
    },
    #[error("runtime reachability references an unresolved scope")]
    UnresolvedScope { scope: ScopeId },
    #[error("runtime reachability references an unresolved local")]
    UnresolvedLocal { local: LocalId },
    #[error("runtime reachability references an unresolved expression")]
    UnresolvedExpression { expression: ExprId },
    #[error("runtime reachability references an unresolved statement")]
    UnresolvedStatement { statement: StmtId },
    #[error("runtime reachability references an unresolved type")]
    UnresolvedType { ty: TypeId },
    #[error("runtime reachability references an unresolved pattern")]
    UnresolvedPattern { pattern: PatternId },
    #[error("runtime reachability exceeds the accepted graph limit")]
    LimitExceeded {
        family: HirRuntimeReachabilityLimitFamily,
        actual: usize,
        limit: usize,
    },
}
```

## 2. Paths

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityPath {
    root: HirRuntimeReachabilityRoot,
    steps: Box<[HirRuntimeReachabilityEdge]>,
}

impl HirRuntimeReachabilityPath {
    pub const fn root(&self) -> &HirRuntimeReachabilityRoot;
    pub const fn steps(&self) -> &[HirRuntimeReachabilityEdge];
}
```

Paths are built through predecessor indices and materialized after closure.

## 3. Sema function classification

```rust
impl CheckedItemRole {
    #[must_use]
    pub fn ordinary_function_emission(
        &self,
        effects: &EffectSet,
    ) -> Option<CheckedOrdinaryFunctionEmission> {
        let Self::Function { execution, suspension } = self else {
            return None;
        };
        Some(match (execution, suspension, effects.is_empty()) {
            (
                CheckedFunctionExecution::DirectFrame,
                CheckedSuspensionRole::NonSuspending,
                true,
            ) => CheckedOrdinaryFunctionEmission::PureDirectFrame,
            (
                CheckedFunctionExecution::DirectFrame,
                CheckedSuspensionRole::NonSuspending,
                false,
            ) => CheckedOrdinaryFunctionEmission::EffectfulDirectFrameUnsupported,
            (
                CheckedFunctionExecution::DirectFrame,
                CheckedSuspensionRole::MaySuspend,
                _,
            ) => CheckedOrdinaryFunctionEmission::SuspendingDirectFrameUnsupported,
            (CheckedFunctionExecution::StreamFactory { .. }, _, _) => {
                CheckedOrdinaryFunctionEmission::StreamFactoryUnsupported
            }
        })
    }
}
```

If current source stores effects on `CheckedItem` rather than the role, the caller passes `checked.effects()`; the classification table remains on the owner enum.

## 4. Compiler preflight

```rust
pub fn validate_reachable_runtime_callables(
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
) -> Result<(), RuntimeReachabilityProjectionError> {
    for executable in reachability.reachable_executables() {
        let HirRuntimeExecutableOwner::Item(owner) = executable else {
            continue;
        };
        let Some(item) = analysis.item(owner) else {
            return Err(RuntimeReachabilityProjectionError::MissingCheckedItem { owner });
        };
        let Some(class) = item
            .role()
            .ordinary_function_emission(item.effects())
        else {
            continue;
        };
        if class == CheckedOrdinaryFunctionEmission::PureDirectFrame {
            continue;
        }
        return Err(RuntimeReachabilityProjectionError::UnsupportedOrdinaryFunction {
            owner,
            reason: class,
            path: reachability
                .first_path(executable)
                .expect("reached executable owns one accepted path")
                .clone(),
            suspension_site: analysis.first_direct_suspension_site(owner),
        });
    }
    Ok(())
}
```

The final implementation must use existing checked effect accessors and avoid duplicating effect inference.

## 5. Typed nominal schema descent

```rust
impl NominalSchemaPath {
    #[must_use]
    pub fn pushed(&self, step: NominalSchemaPathStep) -> Self {
        let mut steps = self.0.to_vec();
        steps.push(step);
        Self(steps.into_boxed_slice())
    }
}
```

```rust
fn type_shape(
    &self,
    ty: &TypeKind,
    path: &NominalSchemaPath,
    /* existing substitutions/stacks */
) -> Result<TypeShape, NominalSchemaProjectionError> {
    match ty {
        TypeKind::Option(inner) => self.type_shape(
            inner,
            &path.pushed(NominalSchemaPathStep::OptionItem),
            /* ... */
        ),
        TypeKind::Vec(inner) | TypeKind::Seq(inner) => self.type_shape(
            inner,
            &path.pushed(NominalSchemaPathStep::SequenceItem),
            /* ... */
        ),
        TypeKind::AcceptedNominal(accepted) if accepted.is_opaque_runtime_type() => {
            Err(NominalSchemaProjectionError::OpaqueLeaf {
                path: path.clone(),
                producer: accepted.producer().clone(),
                semantic_identity: accepted.semantic_identity(),
            })
        }
        /* existing closed-schema cases */
        unsupported => Err(NominalSchemaProjectionError::UnsupportedLeaf {
            path: path.clone(),
            ty: Box::new(unsupported.clone()),
        }),
    }
}
```

Use the repository's actual accepted-nominal variant/accessors; do not add a parallel opaque detector if the existing enum already owns it.

## 6. Runtime projection error mapping

```rust
fn runtime_nominal(
    nominal: &CheckedProjectNominal,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominal, RuntimeSemanticProjectionError> {
    let shape = analysis
        .project_nominal_schema(symbols, nominal)
        .map_err(|source| match source {
            NominalSchemaProjectionError::OpaqueLeaf {
                path,
                producer,
                semantic_identity,
            } => RuntimeSemanticProjectionError::OpaqueProjectNominalLayout {
                nominal: nominal.declaration().clone(),
                path,
                producer,
                semantic_identity,
            },
            source => RuntimeSemanticProjectionError::NominalSchemaProjection {
                nominal: nominal.declaration().qualified_name(),
                source,
            },
        })?;
    let schema = RuntimeSchemaProjection::schema(&shape);
    let layout = RuntimeSchemaProjection::layout_hash(
        &nominal.declaration().qualified_name(),
        &schema,
    )?;
    Ok(RuntimeResolvedNominal::new(
        nominal.declaration().clone(),
        nominal.owner(),
        RuntimeSemanticTypeId::from_bytes(*nominal.identity().as_bytes()),
        layout,
    ))
}
```

No alternate success arm exists.

## 7. Fact admission

```rust
impl RuntimePlanSemanticFactInput {
    fn ensure_owner(
        &self,
        owner: RuntimeSemanticFactOwner,
    ) -> Result<(), RuntimeSemanticFactsError> {
        if self.reachability.contains(owner) {
            Ok(())
        } else {
            Err(RuntimeSemanticFactsError::OwnerOutsideReachability { owner })
        }
    }
}
```

Every `push_*` calls `ensure_owner`. Flow and helper inventories are derived from reached executable owners, not from a separate scan.

## 8. Tooling disposition

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEmissionDisposition {
    Root { kind: HirRuntimeReachabilityRootKind },
    Reachable { path: HirRuntimeReachabilityPath },
    NotSelected,
}
```

This is an owned projection for display/indexing only and is not a compilation input.
