use arcweft_core::entry::RuntimeSchemaLimits;

use super::RuntimeNominalGraphProjectionError as Error;
use crate::types::{TypeProjectionControl, TypeProjectionNodeKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeNominalGraphProjectionLimits {
    pub max_type_nodes: u64,
    pub max_nominal_edges: u64,
    pub max_definitions: u64,
    pub max_fields_and_cases: u64,
    pub max_active_nominal_depth: u64,
}

impl RuntimeNominalGraphProjectionLimits {
    pub const PRODUCTION: Self = Self {
        max_type_nodes: 65_536,
        max_nominal_edges: 16_384,
        max_definitions: 16_384,
        max_fields_and_cases: 65_536,
        max_active_nominal_depth: 64,
    };
}

impl Default for RuntimeNominalGraphProjectionLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNominalGraphProjectionLimitKind {
    TypeNodes,
    NominalEdges,
    Definitions,
    FieldsAndCases,
    ActiveNominalDepth,
    TypeDepth,
    NameBytes,
}

use RuntimeNominalGraphProjectionLimitKind as Kind;

pub(in crate::final_analysis::nominal_schema) struct ProjectionBudget {
    limits: RuntimeNominalGraphProjectionLimits,
    type_nodes: u64,
    nominal_edges: u64,
    definitions: u64,
    fields_and_cases: u64,
}

impl ProjectionBudget {
    pub(in crate::final_analysis::nominal_schema) fn new(
        limits: RuntimeNominalGraphProjectionLimits,
    ) -> Result<Self, Error> {
        let hard = RuntimeNominalGraphProjectionLimits::PRODUCTION;
        for (kind, actual, maximum) in [
            (Kind::TypeNodes, limits.max_type_nodes, hard.max_type_nodes),
            (
                Kind::NominalEdges,
                limits.max_nominal_edges,
                hard.max_nominal_edges,
            ),
            (
                Kind::Definitions,
                limits.max_definitions,
                hard.max_definitions,
            ),
            (
                Kind::FieldsAndCases,
                limits.max_fields_and_cases,
                hard.max_fields_and_cases,
            ),
            (
                Kind::ActiveNominalDepth,
                limits.max_active_nominal_depth,
                hard.max_active_nominal_depth,
            ),
        ] {
            Self::check_limit(kind, actual, maximum)?;
        }
        Ok(Self {
            limits,
            type_nodes: 0,
            nominal_edges: 0,
            definitions: 0,
            fields_and_cases: 0,
        })
    }

    fn check_limit(kind: Kind, observed: u64, maximum: u64) -> Result<(), Error> {
        if observed > maximum {
            Err(Error::Limit {
                kind,
                observed,
                maximum,
            })
        } else {
            Ok(())
        }
    }

    pub(in crate::final_analysis::nominal_schema) fn type_node(
        &mut self,
        depth: u64,
    ) -> Result<(), Error> {
        self.type_nodes = self.type_nodes.saturating_add(1);
        Self::check_limit(Kind::TypeNodes, self.type_nodes, self.limits.max_type_nodes)?;
        Self::check_limit(
            Kind::TypeDepth,
            depth,
            u64::from(RuntimeSchemaLimits::engine_default().max_depth),
        )
    }

    pub(in crate::final_analysis::nominal_schema) fn edge(&mut self) -> Result<(), Error> {
        self.nominal_edges = self.nominal_edges.saturating_add(1);
        Self::check_limit(
            Kind::NominalEdges,
            self.nominal_edges,
            self.limits.max_nominal_edges,
        )
    }

    pub(in crate::final_analysis::nominal_schema) fn definition(
        &mut self,
        depth: u64,
    ) -> Result<(), Error> {
        self.definitions = self.definitions.saturating_add(1);
        Self::check_limit(
            Kind::Definitions,
            self.definitions,
            self.limits.max_definitions,
        )?;
        Self::check_limit(
            Kind::ActiveNominalDepth,
            depth,
            self.limits.max_active_nominal_depth,
        )
    }

    pub(in crate::final_analysis::nominal_schema) fn members(
        &mut self,
        count: usize,
    ) -> Result<(), Error> {
        self.fields_and_cases = self
            .fields_and_cases
            .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        Self::check_limit(
            Kind::FieldsAndCases,
            self.fields_and_cases,
            self.limits.max_fields_and_cases,
        )
    }

    pub(in crate::final_analysis::nominal_schema) fn name(&self, name: &str) -> Result<(), Error> {
        Self::check_limit(
            Kind::NameBytes,
            u64::try_from(name.len()).unwrap_or(u64::MAX),
            u64::from(RuntimeSchemaLimits::engine_default().max_string_bytes),
        )
    }
}

impl TypeProjectionControl for ProjectionBudget {
    type Error = Error;
    fn check(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, depth: u64) -> Result<(), Error> {
        self.type_node(depth)
    }
    fn visit_binding(&mut self) -> Result<(), Error> {
        self.type_node(1)
    }
}
