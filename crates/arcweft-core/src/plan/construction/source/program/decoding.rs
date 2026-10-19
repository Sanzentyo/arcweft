use super::super::expression::RuntimeExpressionSourceHandles;
use super::super::flow::RuntimeFlowSourceHandles;
use super::*;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::declaration_sources::{
    PreparedRuntimePlanDeclarationSources, RuntimePlanDeclarationSourceBytes,
};
use crate::plan::{RuntimeCallableStateTable, RuntimeFunctionSiteTable};

pub(in crate::plan) struct PreparedRuntimeProgramSource<'a> {
    frames: Frames<'a>,
    bounds: Bounds,
    declarations: PreparedRuntimePlanDeclarationSources<'a>,
    functions: functions::PreparedRuntimeFunctionSources<'a>,
    states: callable_states::PreparedRuntimeCallableSources<'a>,
    pure: pure_callables::PreparedRuntimePureCallableSources<'a>,
    content_declarations: content_declarations::PreparedRuntimeContentDeclarationSources<'a>,
    invocations: invocations::PreparedRuntimeInvocationSources<'a>,
    contents: content_plans::PreparedRuntimeContentPlanSources<'a>,
    streams: streams::PreparedRuntimeStreamSources<'a>,
    bodies: function_bodies::PreparedRuntimeFunctionBodySources<'a>,
    lines: line_groups::PreparedRuntimeLineGroupSources<'a>,
    entries: entries::PreparedRuntimeEntrySources<'a>,
}
impl<'a> PreparedRuntimeProgramSource<'a> {
    pub(in crate::plan) fn namespaces(
        bytes: &[u8],
        limits: RuntimeProgramSourceLimits,
    ) -> Result<Namespaces, Error> {
        Ok(Frames::read(bytes, limits)?.namespace)
    }

    /// All context-free children and their combined footprint are admitted
    /// first. Specialization's type-dependent check subsequently borrows the
    /// genuine reserved function/state context and the remaining allowance.
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeProgramSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            let frames = Frames::read(bytes, limits)?;
            meter.charge_work(u64::try_from(bytes.len()).map_err(|_| Error::Arithmetic)?)?;
            let mut bounds = Bounds::new(limits, frames.namespace)?;
            let declaration_bytes = RuntimePlanDeclarationSourceBytes {
                types: frames.get(Section::Types),
                nominals: frames.get(Section::Nominals),
                locals: frames.get(Section::Locals),
                controls: frames.get(Section::Controls),
            };
            let declaration_length = [
                declaration_bytes.types,
                declaration_bytes.nominals,
                declaration_bytes.locals,
                declaration_bytes.controls,
            ]
            .into_iter()
            .try_fold(0_usize, |sum, bytes| {
                sum.checked_add(bytes.len()).ok_or(Error::Arithmetic)
            })?;
            let declarations =
                declaration_bytes.preflight(bounds.declarations(declaration_length), meter)?;
            let [types, _, _, locals, controls] = declarations.counts();
            namespace("types", types, frames.namespace.types)?;
            namespace("locals", locals, frames.namespace.locals)?;
            namespace("controls", controls, frames.namespace.controls)?;
            let elements = declarations
                .types
                .elements()
                .checked_add(declarations.nominals.elements())
                .and_then(|n| n.checked_add(declarations.controls.elements()))
                .ok_or(Error::Arithmetic)?;
            bounds.admit(0, elements, declarations.construction_storage_bytes()?)?;
            let bytes = frames.get(Section::Functions);
            let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
                bytes,
                bounds.functions(bytes.len()),
                resources,
                meter,
            )?;
            namespace(
                "functions",
                functions.row_count(),
                frames.namespace.functions,
            )?;
            bounds.admit(
                functions.pattern_nodes(),
                functions.elements(),
                functions.storage_bytes(),
            )?;
            let bytes = frames.get(Section::States);
            let states = RuntimeCallableStateTable::preflight_sources(
                bytes,
                bounds.states(bytes.len()),
                meter,
            )?;
            namespace("states", states.row_count(), frames.namespace.states)?;
            bounds.admit(0, states.elements(), states.storage_bytes())?;
            let bytes = frames.get(Section::Pure);
            let pure = pure_callables::PreparedRuntimePureCallableSources::preflight(
                bytes,
                bounds.pure(bytes.len()),
                meter,
            )?;
            bounds.admit(0, pure.elements(), pure.storage_bytes())?;
            let bytes = frames.get(Section::ContentDeclarations);
            let content_declarations =
                content_declarations::PreparedRuntimeContentDeclarationSources::preflight(
                    bytes,
                    bounds.content_declarations(bytes.len()),
                    meter,
                )?;
            bounds.admit(
                0,
                content_declarations.elements(),
                content_declarations.storage_bytes(),
            )?;
            let bytes = frames.get(Section::Invocations);
            let invocations = invocations::PreparedRuntimeInvocationSources::preflight(
                bytes,
                bounds.invocations(bytes.len()),
                meter,
            )?;
            bounds.admit(0, invocations.elements(), invocations.storage_bytes())?;
            let bytes = frames.get(Section::Contents);
            let contents = content_plans::PreparedRuntimeContentPlanSources::preflight(
                bytes,
                bounds.contents(bytes.len()),
                resources,
                meter,
            )?;
            bounds.admit(
                contents.node_count(),
                contents.elements(),
                contents.storage_bytes(),
            )?;
            let bytes = frames.get(Section::Streams);
            let streams = streams::PreparedRuntimeStreamSources::preflight(
                bytes,
                bounds.streams(bytes.len()),
                resources,
                meter,
            )?;
            bounds.admit(
                streams.node_count(),
                streams.elements(),
                streams.storage_bytes(),
            )?;
            let bytes = frames.get(Section::Bodies);
            let bodies = function_bodies::PreparedRuntimeFunctionBodySources::preflight(
                bytes,
                bounds.bodies(bytes.len()),
                resources,
                meter,
            )?;
            bounds.admit(
                bodies.node_count(),
                bodies.elements(),
                bodies.storage_bytes(),
            )?;
            let bytes = frames.get(Section::Lines);
            let lines = line_groups::PreparedRuntimeLineGroupSources::preflight(
                bytes,
                bounds.lines(bytes.len()),
                resources,
                meter,
            )?;
            bounds.admit(lines.node_count(), lines.elements(), lines.storage_bytes())?;
            let bytes = frames.get(Section::Entries);
            let entries = entries::PreparedRuntimeEntrySources::preflight(
                bytes,
                bounds.entries(bytes.len()),
                meter,
            )?;
            bounds.admit(
                entries.node_count(),
                entries.elements(),
                entries.storage_bytes(),
            )?;
            Ok(Self {
                frames,
                bounds,
                declarations,
                functions,
                states,
                pure,
                content_declarations,
                invocations,
                contents,
                streams,
                bodies,
                lines,
                entries,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }

    pub(in crate::plan) fn materialize(
        self,
        meter: TaskSemanticMeter,
        resources: &mut ValueResourceAdmission,
    ) -> Result<OriginalRuntimeProgramSource, Error> {
        let Self {
            frames,
            mut bounds,
            declarations,
            functions,
            states,
            pure,
            content_declarations,
            invocations,
            contents,
            streams,
            bodies,
            lines,
            entries,
        } = self;
        let mut source = declarations.into_construction(meter)?;
        let result = (|| {
            source
                .reserve_project_sites(frames.namespace.projects)
                .map_err(RuntimeTaskPlanSealError::from)?;
            let functions = functions.reserve(&mut source, resources)?;
            let states = states.materialize(&mut source, &functions)?;
            let bytes = frames.get(Section::Specializations);
            let specializations = source
                .preflight_specialization_sources(bytes, bounds.specializations(bytes.len()))?;
            bounds.admit(
                0,
                specializations.elements(),
                specializations.storage_bytes(),
            )?;
            let specializations = specializations.materialize(&mut source, &states)?;
            namespace(
                "specializations",
                specializations.len(),
                frames.namespace.specializations,
            )?;
            let callables = pure.reserve(&mut source, &functions)?;
            namespace("helpers", callables.helpers.len(), frames.namespace.helpers)?;
            namespace("methods", callables.methods.len(), frames.namespace.methods)?;
            let attempts = content_declarations.reserve(&mut source)?;
            namespace(
                "templates",
                source.builder.dialogue_content.template_count(),
                frames.namespace.templates,
            )?;
            namespace("attempts", attempts.len(), frames.namespace.attempts)?;
            let defers = invocations.materialize(&mut source, &functions)?;
            namespace("defers", defers.len(), frames.namespace.defers)?;
            let mut tasks = Vec::with_capacity(frames.namespace.tasks);
            for _ in 0..frames.namespace.tasks {
                tasks.push(source.builder.reserve_task_plan()?);
            }
            let expression = RuntimeExpressionSourceHandles {
                states: &states,
                specializations: &specializations,
                helpers: &callables.helpers,
                methods: &callables.methods,
                attempts: &attempts,
            };
            let content_handles =
                contents.materialize(&mut source, &functions, &expression, resources)?;
            namespace("contents", content_handles.len(), frames.namespace.contents)?;
            let flow = RuntimeFlowSourceHandles {
                expression: &expression,
                contents: &content_handles,
                defers: &defers,
                tasks: &tasks,
            };
            streams.materialize(&mut source, &expression, resources)?;
            bodies.materialize(&mut source, &functions, &flow, resources)?;
            let groups = lines.materialize(&mut source, &flow, resources)?;
            namespace("groups", groups.len(), frames.namespace.groups)?;
            contents.bind_line_groups(&mut source, &content_handles, &groups)?;
            entries.materialize(&mut source)?;
            Ok((
                functions,
                groups,
                tasks.into_boxed_slice(),
                bounds.footprint,
            ))
        })();
        let (functions, groups, tasks, footprint) =
            result.map_err(|error: Error| error.reject(&mut source.meter))?;
        Ok(OriginalRuntimeProgramSource {
            source,
            functions,
            groups,
            tasks,
            footprint,
        })
    }
}
