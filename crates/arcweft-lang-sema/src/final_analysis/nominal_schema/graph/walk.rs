//! Source-ordered, stack-independent projection of an instantiated schema graph.

use arcweft_core::entry::RuntimeNominalDeclarationId;

use super::{
    Error, InstantiatedRustTypeMetadata, NominalGraphProjection, RuntimeMapKind,
    RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaIdentity,
    RuntimeOpaqueTypeOwner, RuntimeRecordFieldId, RuntimeSchemaValueField, Schema,
    SemanticTypeDigest, TypeKind, VisitState, project::ProjectBody, storage::ProjectionStorage,
    variants::ClosedCase,
};
use crate::final_analysis::nominal_schema::NominalSchemaPathStep;

pub(super) enum Task<'walk> {
    Type(&'walk TypeKind, u64),
    Types(&'walk [TypeKind], u64),
    LeaveType,
    BuildSchema(Assembly),
    Name(&'walk str),
    CountPayload(u64),
    Field {
        ordinal: usize,
        name: Option<&'walk str>,
    },
    ValueField {
        ordinal: usize,
        name: &'walk str,
    },
    Case {
        ordinal: u32,
        name: &'walk str,
        has_payload: bool,
    },
    ProjectBody(ProjectBody<'walk>),
    RustBody(Definition<'walk>, u64),
    ProjectField(super::project::ProjectField<'walk>),
    ProjectCase(super::project::ProjectCase<'walk>),
    ClosedCase(ClosedCase<'walk>),
    FinishDefinition(Definition<'walk>, Body),
    LeavePath,
}

pub(super) enum Assembly {
    Seq,
    Map(RuntimeMapKind),
    Array(usize),
    Option,
    Result,
    Tuple(usize),
    Choice(usize),
    RecordValue(usize),
    ExactOpaque {
        owner: RuntimeOpaqueTypeOwner,
        arguments: usize,
    },
}

pub(super) enum Body {
    Record(RuntimeNominalRecordShape, usize),
    Variant(usize),
}

pub(super) struct Definition<'walk> {
    pub(super) semantic: SemanticTypeDigest,
    pub(super) identity: RuntimeNominalSchemaIdentity,
    pub(super) declaration: RuntimeNominalDeclarationId,
    pub(super) arguments: usize,
    pub(super) rust: Option<(usize, &'walk InstantiatedRustTypeMetadata)>,
}

pub(super) struct Walk<'walk> {
    pub(super) storage: &'walk ProjectionStorage,
    tasks: Vec<Task<'walk>>,
    schemas: Vec<Schema>,
    fields: Vec<RuntimeNominalSchemaField>,
    value_fields: Vec<RuntimeSchemaValueField>,
    cases: Vec<RuntimeNominalSchemaCase>,
    path: Vec<NominalSchemaPathStep>,
    root_rust_slot: Option<usize>,
}

impl<'walk> Walk<'walk> {
    pub(super) fn run<'source: 'walk>(
        storage: &'walk ProjectionStorage,
        projection: &mut NominalGraphProjection<'source>,
        root: &'walk TypeKind,
    ) -> Result<(Schema, Option<usize>), Error> {
        let mut walk = Self {
            storage,
            tasks: vec![Task::Type(root, 1)],
            schemas: Vec::new(),
            fields: Vec::new(),
            value_fields: Vec::new(),
            cases: Vec::new(),
            path: Vec::new(),
            root_rust_slot: None,
        };
        while let Some(task) = walk.tasks.pop() {
            if let Err(error) = walk.step(projection, task) {
                return Err(match error {
                    Error::Project(error) => Error::Project(
                        walk.path
                            .into_iter()
                            .rev()
                            .fold(error, |error, step| error.within_step(step)),
                    ),
                    other => other,
                });
            }
        }
        let root = walk.pop_schema();
        debug_assert!(walk.schemas.is_empty());
        debug_assert!(
            walk.fields.is_empty() && walk.value_fields.is_empty() && walk.cases.is_empty()
        );
        debug_assert!(walk.path.is_empty() && projection.active_depth == 0);
        Ok((root, walk.root_rust_slot))
    }

    pub(super) fn push(&mut self, task: Task<'walk>) {
        self.tasks.push(task);
    }

    pub(super) fn emit(&mut self, schema: Schema) {
        self.schemas.push(schema);
    }

    pub(super) fn push_field(
        &mut self,
        ordinal: usize,
        name: Option<&'walk str>,
        ty: &'walk TypeKind,
        depth: u64,
    ) {
        self.push(Task::Field { ordinal, name });
        self.push(Task::Type(ty, depth));
    }

    pub(super) fn push_value_field(
        &mut self,
        ordinal: usize,
        name: &'walk str,
        ty: &'walk TypeKind,
        depth: u64,
    ) {
        self.push(Task::ValueField { ordinal, name });
        self.push(Task::Type(ty, depth));
        self.push(Task::Name(name));
    }

    /// The original declaration supplied a fresh type. Charge the new node
    /// before reserving a slot and retain its source path until its walk ends.
    pub(super) fn visit_instantiated<'source: 'walk>(
        &mut self,
        projection: &mut NominalGraphProjection<'source>,
        ty: TypeKind,
        depth: u64,
        path: Option<NominalSchemaPathStep>,
    ) -> Result<(), Error> {
        if let Some(path) = path {
            self.path.push(path);
            self.push(Task::LeavePath);
        }
        self.enter_type(projection, depth)?;
        let (_, ty) = self.storage.types.insert(ty);
        projection.visit_type(self, ty, depth)
    }

    fn enter_type(
        &mut self,
        projection: &mut NominalGraphProjection<'_>,
        depth: u64,
    ) -> Result<(), Error> {
        if let Some((budget, control)) = &mut projection.project_budget {
            budget.enter_node(*control)?;
        }
        projection.budget.type_node(depth)?;
        self.push(Task::LeaveType);
        Ok(())
    }

    fn step<'source: 'walk>(
        &mut self,
        projection: &mut NominalGraphProjection<'source>,
        task: Task<'walk>,
    ) -> Result<(), Error> {
        match task {
            Task::Type(ty, depth) => {
                self.enter_type(projection, depth)?;
                projection.visit_type(self, ty, depth)?;
            }
            Task::Types(types, depth) => {
                if let Some((ty, rest)) = types.split_first() {
                    self.push(Task::Types(rest, depth));
                    self.push(Task::Type(ty, depth));
                }
            }
            Task::LeaveType => {
                if let Some((budget, _)) = &mut projection.project_budget {
                    budget.leave_node();
                }
            }
            Task::BuildSchema(assembly) => {
                let schema = self.assemble(assembly)?;
                self.emit(schema);
            }
            Task::Name(name) => projection.budget.name(name)?,
            Task::CountPayload(depth) => projection.budget.type_node(depth)?,
            Task::Field { ordinal, name } => {
                let schema = self.pop_schema();
                self.fields.push(RuntimeNominalSchemaField::new(
                    field_id(ordinal),
                    name.map(str::to_owned),
                    schema,
                ));
            }
            Task::ValueField { ordinal, name } => {
                let schema = self.pop_schema();
                self.value_fields.push(RuntimeSchemaValueField::new(
                    field_id(ordinal),
                    name.to_owned(),
                    schema,
                ));
            }
            Task::Case {
                ordinal,
                name,
                has_payload,
            } => {
                let payload = has_payload.then(|| self.pop_schema());
                self.cases.push(RuntimeNominalSchemaCase::new(
                    ordinal,
                    name.to_owned(),
                    payload,
                ));
            }
            Task::ProjectBody(body) => body.schedule(projection, self)?,
            Task::RustBody(definition, depth) => {
                NominalGraphProjection::schedule_rust_body(self, definition, depth);
            }
            Task::ProjectField(field) => field.visit(projection, self)?,
            Task::ProjectCase(case) => case.visit(projection, self)?,
            Task::ClosedCase(case) => case.visit(projection, self)?,
            Task::FinishDefinition(definition, body) => {
                self.finish_definition(projection, definition, body)?;
            }
            Task::LeavePath => {
                self.path
                    .pop()
                    .expect("a field or variant payload path is active");
            }
        }
        Ok(())
    }

    fn pop_schema(&mut self) -> Schema {
        self.schemas
            .pop()
            .expect("child schema was projected before assembly")
    }

    fn assemble(&mut self, assembly: Assembly) -> Result<Schema, Error> {
        Ok(match assembly {
            Assembly::Seq => Schema::Seq(Box::new(self.pop_schema())),
            Assembly::Map(kind) => {
                let value = Box::new(self.pop_schema());
                let key = Box::new(self.pop_schema());
                Schema::Map { kind, key, value }
            }
            Assembly::Array(length) => Schema::Array {
                item: Box::new(self.pop_schema()),
                length: u64::try_from(length)
                    .map_err(|_| crate::types::TypeInstantiationError::EncodingLengthOverflow)?,
            },
            Assembly::Option => Schema::option(self.pop_schema()),
            Assembly::Result => {
                let error = self.pop_schema();
                Schema::result(self.pop_schema(), error)
            }
            Assembly::Tuple(count) => Schema::Tuple(take_tail(&mut self.schemas, count)),
            Assembly::Choice(count) => Schema::Choice(take_tail(&mut self.schemas, count)),
            Assembly::RecordValue(count) => Schema::RecordValue {
                fields: take_tail(&mut self.value_fields, count),
            },
            Assembly::ExactOpaque { owner, arguments } => Schema::ExactOpaque {
                owner,
                arguments: take_tail(&mut self.schemas, arguments),
            },
        })
    }

    fn finish_definition(
        &mut self,
        projection: &mut NominalGraphProjection<'_>,
        definition: Definition<'walk>,
        body: Body,
    ) -> Result<(), Error> {
        let body = match body {
            Body::Record(shape, count) => RuntimeNominalSchemaBody::Record {
                shape,
                fields: take_tail(&mut self.fields, count),
            },
            Body::Variant(count) => RuntimeNominalSchemaBody::Variant {
                cases: take_tail(&mut self.cases, count),
            },
        };
        let arguments = take_tail(&mut self.schemas, definition.arguments);
        let mut row = RuntimeNominalSchemaDefinition::new(
            definition.declaration,
            definition.identity.clone(),
            arguments,
            body,
        );
        if let Some((slot, rust)) = definition.rust {
            if let Some(codec) = projection.rust_default_codec(rust, row.body())? {
                row = row.with_data_codec(codec);
            }
            if projection.active_depth == 1 {
                self.root_rust_slot = Some(slot);
            }
        }
        projection.definitions.push(row);
        projection.active_depth -= 1;
        projection.states.insert(
            definition.semantic,
            VisitState::Complete(definition.identity.clone()),
        );
        self.emit(Schema::NominalRef(definition.identity));
        Ok(())
    }
}

fn take_tail<T>(stack: &mut Vec<T>, count: usize) -> Box<[T]> {
    stack.split_off(stack.len() - count).into_boxed_slice()
}

fn field_id(ordinal: usize) -> RuntimeRecordFieldId {
    RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).expect("fields were bounded")
}
