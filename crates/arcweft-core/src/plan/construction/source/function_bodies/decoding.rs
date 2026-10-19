use super::super::RuntimePlanSourceConstruction;
use super::super::flow::{PreparedRuntimeFlowSource, RuntimeFlowSourceHandles};
use super::reading::{self, Cursor, Row};
use super::{BodyOwner, Counts, Error, PreparedRuntimeFunctionBodySources, charge, namespace};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFunctionSiteBodyKind,
    RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteSeedId, RuntimePlanBuildError,
};
use crate::value::RuntimeExpr;

pub(super) struct CheckedRow<'a> {
    row: Row<'a>,
    effects: RuntimeEffectSet,
}
impl PreparedRuntimeFunctionBodySources<'_> {
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        functions: &[RuntimeFunctionSiteSeedId],
        handles: &RuntimeFlowSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.flow)?;
            namespace(
                "function declarations",
                self.limits.functions,
                source.builder.function_sites.len(),
            )?;
            namespace("function handles", self.limits.functions, functions.len())?;
            for (index, function) in functions.iter().enumerate() {
                if function
                    .resolve(&source.builder.issuer)
                    .is_none_or(|(id, ..)| id.get().get() as usize != index + 1)
                {
                    return Err(Error::Handle);
                }
            }
            charge(&mut source.meter, self.bytes.len())?;
            let mut rows = Vec::with_capacity(self.rows);
            let (_, footprint) = reading::inspect(
                self.bytes,
                self.limits,
                resources,
                &mut source.meter,
                |row| rows.push(row),
            )?;
            debug_assert_eq!(footprint, self.footprint);
            let mut checked = Vec::with_capacity(self.rows);
            // Validate all symbolic definitions/permissions before the first
            // expression/Flow value, including forward-referenced functions.
            for (index, row) in rows.into_iter().enumerate() {
                let (definition, defined) = match row.owner {
                    BodyOwner::Function(index) => {
                        let declaration = &source.builder.function_sites[index];
                        (declaration.definition, declaration.body.is_some())
                    }
                    BodyOwner::Helper(index) => {
                        let declaration = &source.builder.pure_helpers[index];
                        (declaration.definition, declaration.body.is_some())
                    }
                    BodyOwner::Method(index) => {
                        let declaration = &source.builder.trait_methods[index];
                        (declaration.definition, declaration.body.is_some())
                    }
                };
                if definition.as_bytes() != &row.definition {
                    return Err(Error::Definition { row: index });
                }
                if let BodyOwner::Function(index) = row.owner
                    && source.builder.function_sites[index].body_kind != row.kind
                {
                    return Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch {
                        site: functions[index].id(),
                        expected: source.builder.function_sites[index].body_kind,
                        actual: row.kind,
                    }
                    .into());
                }
                if defined {
                    let error = match row.owner {
                        BodyOwner::Function(index) => {
                            RuntimePlanBuildError::DuplicateFunctionSiteDefinition {
                                site: functions[index].id(),
                            }
                        }
                        BodyOwner::Helper(index) => {
                            RuntimePlanBuildError::DuplicatePureHelperDefinition {
                                helper: handles.expression.helpers[index]
                                    .resolve(&source.builder.issuer)
                                    .expect("authenticated helper")
                                    .0,
                            }
                        }
                        BodyOwner::Method(index) => {
                            RuntimePlanBuildError::DuplicateTraitMethodDefinition {
                                method: handles.expression.methods[index]
                                    .resolve(&source.builder.issuer)
                                    .expect("authenticated method")
                                    .0,
                            }
                        }
                    };
                    return Err(error.into());
                }
                let mut cursor = Cursor::new(row.effects);
                let count = cursor.u32()? as usize;
                let effects = (0..count)
                    .map(|_| Ok(arcweft_id::EffectId::parse(cursor.text()?)?))
                    .collect::<Result<Vec<_>, Error>>()?;
                cursor.finish()?;
                let effects = RuntimeEffectSet::try_from_effects(effects)?;
                if let BodyOwner::Function(index) = row.owner
                    && !source.builder.function_sites[index]
                        .effects
                        .covers(&effects)
                {
                    return Err(RuntimePlanBuildError::FunctionSiteEffectSetMismatch {
                        site: functions[index].id(),
                    }
                    .into());
                }
                checked.push(CheckedRow { row, effects });
            }
            for checked in checked {
                let CheckedRow { row, effects } = checked;
                let counts = Counts {
                    limits: self.limits,
                    footprint: row.prefix,
                };
                let body = match row.kind {
                    RuntimeFunctionSiteBodyKind::Expression => {
                        RuntimeFunctionSiteBodySeed::Expression(
                            RuntimeExpr::preflight_sources(
                                row.payload,
                                counts.expression(row.payload.len()),
                                resources,
                                &mut source.meter,
                            )?
                            .materialize(
                                source,
                                handles.expression,
                                resources,
                            )?,
                        )
                    }
                    RuntimeFunctionSiteBodyKind::Executable => {
                        let ops = PreparedRuntimeFlowSource::preflight(
                            row.payload,
                            counts.flow(row.payload.len()),
                            resources,
                            &mut source.meter,
                        )?
                        .materialize(source, handles, resources)?;
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                            effects,
                            ops: ops.into_boxed_slice(),
                        })
                    }
                };
                match (row.owner, body) {
                    (BodyOwner::Function(index), body) => source
                        .builder
                        .define_function_site_seed(&functions[index], body)?,
                    (BodyOwner::Helper(index), RuntimeFunctionSiteBodySeed::Expression(body)) => {
                        source
                            .builder
                            .define_pure_helper_seed(&handles.expression.helpers[index], body)?
                    }
                    (BodyOwner::Method(index), RuntimeFunctionSiteBodySeed::Expression(body)) => {
                        source
                            .builder
                            .define_trait_method_seed(&handles.expression.methods[index], body)?
                    }
                    (owner, _) => {
                        return Err(Error::PureBodyKind {
                            owner: owner.label(),
                        });
                    }
                }
            }
            Ok(())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
