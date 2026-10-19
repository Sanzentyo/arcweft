use super::super::{
    callable_specializations::RuntimeSpecializationSourceLimits,
    callable_states::RuntimeCallableSourceLimits,
    content_declarations::RuntimeContentDeclarationSourceLimits,
    content_plans::RuntimeContentPlanSourceLimits, entries::RuntimeEntrySourceLimits,
    expression::RuntimeExpressionSourceLimits, flow::RuntimeFlowSourceLimits,
    function_bodies::RuntimeFunctionBodySourceLimits, functions::RuntimeFunctionSourceLimits,
    invocations::RuntimeInvocationSourceLimits, line_groups::RuntimeLineGroupSourceLimits,
    pure_callables::RuntimePureCallableSourceLimits, streams::RuntimeStreamSourceLimits,
};
use super::{Error, Footprint, Namespaces, RuntimeProgramSourceLimits, limit};
use crate::plan::declaration_sources::RuntimePlanDeclarationSourceLimits;

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub limits: RuntimeProgramSourceLimits,
    pub namespace: Namespaces,
    pub footprint: Footprint,
}
impl Bounds {
    pub fn new(limits: RuntimeProgramSourceLimits, namespace: Namespaces) -> Result<Self, Error> {
        let elements = namespace.validate(limits)?;
        let storage = size_of::<super::PreparedRuntimeProgramSource<'_>>()
            .checked_add(size_of::<super::OriginalRuntimeProgramSource>())
            .and_then(|size| {
                namespace
                    .tasks
                    .checked_mul(
                        2 * (size_of::<crate::plan::RuntimeTaskPlanBuildCoordinate>()
                            + size_of::<super::super::super::task_plans::RuntimeTaskPlanDefinition>(
                            )),
                    )
                    .and_then(|tasks| size.checked_add(tasks))
            })
            .and_then(|size| {
                namespace
                    .projects
                    .checked_mul(
                        2 * size_of::<Option<crate::plan::RuntimeProjectCallSite>>()
                            + size_of::<crate::plan::RuntimeProjectCallSiteSeedId>(),
                    )
                    .and_then(|projects| size.checked_add(projects))
            })
            .ok_or(Error::Arithmetic)?;
        let mut bounds = Self {
            limits,
            namespace,
            footprint: Footprint::default(),
        };
        bounds.admit(0, elements, storage)?;
        Ok(bounds)
    }
    pub fn admit(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(elements)
            .ok_or(Error::Arithmetic)?;
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(storage)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn nodes(self) -> u32 {
        self.limits.nodes - u32::try_from(self.footprint.nodes).expect("admitted nodes")
    }
    fn elements(self) -> u32 {
        self.limits.elements - u32::try_from(self.footprint.elements).expect("admitted elements")
    }
    fn storage(self) -> usize {
        self.limits.decoded_bytes - self.footprint.storage
    }
    pub fn declarations(self, encoded_bytes: usize) -> RuntimePlanDeclarationSourceLimits {
        RuntimePlanDeclarationSourceLimits {
            types: self.limits.rows,
            nominals: self.limits.rows,
            locals: self.limits.rows,
            controls: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            policy_depth: self.limits.policy_depth,
        }
    }
    pub fn functions(self, encoded_bytes: usize) -> RuntimeFunctionSourceLimits {
        RuntimeFunctionSourceLimits {
            rows: self.limits.rows,
            elements: self.elements(),
            pattern_nodes: self.nodes(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
            local_count: self.namespace.locals,
        }
    }
    pub fn states(self, encoded_bytes: usize) -> RuntimeCallableSourceLimits {
        RuntimeCallableSourceLimits {
            rows: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
            function_count: self.namespace.functions,
        }
    }
    pub fn specializations(self, encoded_bytes: usize) -> RuntimeSpecializationSourceLimits {
        RuntimeSpecializationSourceLimits {
            rows: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
            state_count: self.namespace.states,
        }
    }
    pub fn pure(self, encoded_bytes: usize) -> RuntimePureCallableSourceLimits {
        RuntimePureCallableSourceLimits {
            helpers: self.limits.rows,
            methods: self.limits.rows,
            bindings: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
            local_count: self.namespace.locals,
            function_count: self.namespace.functions,
        }
    }
    pub fn content_declarations(
        self,
        encoded_bytes: usize,
    ) -> RuntimeContentDeclarationSourceLimits {
        RuntimeContentDeclarationSourceLimits {
            templates: self.limits.rows,
            attempts: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
        }
    }
    pub fn invocations(self, encoded_bytes: usize) -> RuntimeInvocationSourceLimits {
        RuntimeInvocationSourceLimits {
            flows: self.limits.rows,
            schemas: self.limits.rows,
            flow_executables: self.limits.rows,
            callables: self.limits.rows,
            defers: self.limits.rows,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: self.namespace.types,
            function_count: self.namespace.functions,
        }
    }
    pub fn expression(self, encoded_bytes: usize) -> RuntimeExpressionSourceLimits {
        let n = self.namespace;
        RuntimeExpressionSourceLimits {
            nodes: self.nodes(),
            depth: self.limits.depth,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            type_count: n.types,
            local_count: n.locals,
            state_count: n.states,
            specialization_count: n.specializations,
            helper_count: n.helpers,
            method_count: n.methods,
            template_count: n.templates,
            attempt_count: n.attempts,
        }
    }
    pub fn flow(self, encoded_bytes: usize) -> RuntimeFlowSourceLimits {
        RuntimeFlowSourceLimits {
            nodes: self.nodes(),
            depth: self.limits.depth,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            task_count: self.namespace.tasks,
            content_count: self.namespace.contents,
            defer_count: self.namespace.defers,
            project_count: self.namespace.projects,
            expression: self.expression(encoded_bytes),
        }
    }
    pub fn contents(self, encoded_bytes: usize) -> RuntimeContentPlanSourceLimits {
        RuntimeContentPlanSourceLimits {
            rows: self.limits.rows,
            nodes: self.nodes(),
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            function_count: self.namespace.functions,
            group_count: self.namespace.groups,
            expression: self.expression(encoded_bytes),
        }
    }
    pub fn streams(self, encoded_bytes: usize) -> RuntimeStreamSourceLimits {
        RuntimeStreamSourceLimits {
            rows: self.limits.rows,
            nodes: self.nodes(),
            depth: self.limits.depth,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            expression: self.expression(encoded_bytes),
        }
    }
    pub fn bodies(self, encoded_bytes: usize) -> RuntimeFunctionBodySourceLimits {
        RuntimeFunctionBodySourceLimits {
            rows: self.limits.rows,
            functions: self.namespace.functions,
            nodes: self.nodes(),
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            flow: self.flow(encoded_bytes),
        }
    }
    pub fn lines(self, encoded_bytes: usize) -> RuntimeLineGroupSourceLimits {
        RuntimeLineGroupSourceLimits {
            groups: self.limits.rows,
            nodes: self.nodes(),
            depth: self.limits.depth,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
            opaque_producer_bytes: self.limits.opaque_producer_bytes,
            flow: self.flow(encoded_bytes),
        }
    }
    pub fn entries(self, encoded_bytes: usize) -> RuntimeEntrySourceLimits {
        RuntimeEntrySourceLimits {
            entries: self.limits.rows,
            routes: self.limits.rows,
            commands: self.limits.rows,
            schema_nodes: self.nodes(),
            schema_depth: self.limits.depth,
            elements: self.elements(),
            encoded_bytes,
            decoded_bytes: self.storage(),
        }
    }
}
