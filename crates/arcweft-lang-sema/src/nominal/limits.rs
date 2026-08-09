//! Bounded nominal-resolution and aggregation budgets.

/// Per-reference budget that can be crossed while resolving a final-HIR type graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalResolutionLimitKind {
    TypeNodesPerReference,
    RecursiveTypeDepth,
    GenericArgumentsPerApplication,
    AliasExpansionDepth,
    AliasExpansionNodes,
    DiagnosticsPerTypeReference,
    RelatedLabelsPerDiagnostic,
    WorkPerReference,
}

/// Immutable limits for one recursive nominal-resolution operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalResolutionLimits {
    type_nodes_per_reference: u64,
    recursive_type_depth: u16,
    generic_arguments_per_application: u16,
    alias_expansion_depth: u16,
    alias_expansion_nodes: u64,
    diagnostics_per_type_reference: u16,
    related_labels_per_diagnostic: u16,
    work_per_reference: u64,
}

/// Invalid per-reference resolver limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalResolutionLimitsError {
    Zero {
        kind: NominalResolutionLimitKind,
    },
    AboveHardCeiling {
        kind: NominalResolutionLimitKind,
        value: u64,
        ceiling: u64,
    },
    DiagnosticWorkInconsistent {
        diagnostics: u16,
        related_labels: u16,
        work: u64,
    },
}

/// Project-level aggregation budget.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalAggregationLimitKind {
    DiagnosticsPerDocument,
    DiagnosticsPerProject,
    WorkPerProject,
}

/// Immutable limits for combining nominal reports.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NominalAggregationLimits {
    diagnostics_per_document: u16,
    diagnostics_per_project: u16,
    work_per_project: u64,
}

/// Invalid project-level aggregation limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalAggregationLimitsError {
    Zero {
        kind: NominalAggregationLimitKind,
    },
    AboveHardCeiling {
        kind: NominalAggregationLimitKind,
        value: u64,
        ceiling: u64,
    },
    DocumentDiagnosticsExceedProject {
        per_document: u16,
        per_project: u16,
    },
    DiagnosticWorkInconsistent {
        diagnostics: u16,
        work: u64,
    },
}

/// Accepted-catalog collection budget.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalCatalogLimitKind {
    ExactRecords,
    OpenRules,
}

/// Immutable limits for exact accepted records and explicit open rules.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalCatalogLimits {
    exact_records: u16,
    open_rules: u16,
}

/// Invalid accepted nominal catalog limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedNominalCatalogLimitsError {
    Zero {
        kind: AcceptedNominalCatalogLimitKind,
    },
    AboveHardCeiling {
        kind: AcceptedNominalCatalogLimitKind,
        value: u64,
        ceiling: u64,
    },
}

impl NominalResolutionLimits {
    /// Production resolver limits fixed by the nominal-resolution contract.
    pub const PRODUCTION: Self = Self {
        type_nodes_per_reference: 4_096,
        recursive_type_depth: 256,
        generic_arguments_per_application: 256,
        alias_expansion_depth: 64,
        alias_expansion_nodes: 16_384,
        diagnostics_per_type_reference: 32,
        related_labels_per_diagnostic: 32,
        work_per_reference: 65_536,
    };

    /// Validates a custom limit set against the compiled resolver schema.
    #[allow(
        clippy::too_many_arguments,
        reason = "the final contract defines one scalar for each independently charged resolver resource"
    )]
    pub const fn try_new(
        type_nodes_per_reference: u64,
        recursive_type_depth: u16,
        generic_arguments_per_application: u16,
        alias_expansion_depth: u16,
        alias_expansion_nodes: u64,
        diagnostics_per_type_reference: u16,
        related_labels_per_diagnostic: u16,
        work_per_reference: u64,
    ) -> Result<Self, NominalResolutionLimitsError> {
        let values = [
            (
                NominalResolutionLimitKind::TypeNodesPerReference,
                type_nodes_per_reference,
                Self::PRODUCTION.type_nodes_per_reference,
            ),
            (
                NominalResolutionLimitKind::RecursiveTypeDepth,
                recursive_type_depth as u64,
                Self::PRODUCTION.recursive_type_depth as u64,
            ),
            (
                NominalResolutionLimitKind::GenericArgumentsPerApplication,
                generic_arguments_per_application as u64,
                Self::PRODUCTION.generic_arguments_per_application as u64,
            ),
            (
                NominalResolutionLimitKind::AliasExpansionDepth,
                alias_expansion_depth as u64,
                Self::PRODUCTION.alias_expansion_depth as u64,
            ),
            (
                NominalResolutionLimitKind::AliasExpansionNodes,
                alias_expansion_nodes,
                Self::PRODUCTION.alias_expansion_nodes,
            ),
            (
                NominalResolutionLimitKind::DiagnosticsPerTypeReference,
                diagnostics_per_type_reference as u64,
                Self::PRODUCTION.diagnostics_per_type_reference as u64,
            ),
            (
                NominalResolutionLimitKind::RelatedLabelsPerDiagnostic,
                related_labels_per_diagnostic as u64,
                Self::PRODUCTION.related_labels_per_diagnostic as u64,
            ),
            (
                NominalResolutionLimitKind::WorkPerReference,
                work_per_reference,
                Self::PRODUCTION.work_per_reference,
            ),
        ];
        let mut index = 0;
        while index < values.len() {
            let (kind, value, ceiling) = values[index];
            if value == 0 {
                return Err(NominalResolutionLimitsError::Zero { kind });
            }
            if value > ceiling {
                return Err(NominalResolutionLimitsError::AboveHardCeiling {
                    kind,
                    value,
                    ceiling,
                });
            }
            index += 1;
        }

        // Both operands are `u16`, so the widened multiplication cannot overflow.
        let minimum_diagnostic_work =
            (diagnostics_per_type_reference as u64) * ((related_labels_per_diagnostic as u64) + 1);
        if work_per_reference < minimum_diagnostic_work {
            return Err(NominalResolutionLimitsError::DiagnosticWorkInconsistent {
                diagnostics: diagnostics_per_type_reference,
                related_labels: related_labels_per_diagnostic,
                work: work_per_reference,
            });
        }

        Ok(Self {
            type_nodes_per_reference,
            recursive_type_depth,
            generic_arguments_per_application,
            alias_expansion_depth,
            alias_expansion_nodes,
            diagnostics_per_type_reference,
            related_labels_per_diagnostic,
            work_per_reference,
        })
    }

    pub const fn type_nodes_per_reference(self) -> u64 {
        self.type_nodes_per_reference
    }

    pub const fn recursive_type_depth(self) -> u16 {
        self.recursive_type_depth
    }

    pub const fn generic_arguments_per_application(self) -> u16 {
        self.generic_arguments_per_application
    }

    pub const fn alias_expansion_depth(self) -> u16 {
        self.alias_expansion_depth
    }

    pub const fn alias_expansion_nodes(self) -> u64 {
        self.alias_expansion_nodes
    }

    pub const fn diagnostics_per_type_reference(self) -> u16 {
        self.diagnostics_per_type_reference
    }

    pub const fn related_labels_per_diagnostic(self) -> u16 {
        self.related_labels_per_diagnostic
    }

    pub const fn work_per_reference(self) -> u64 {
        self.work_per_reference
    }
}

impl NominalAggregationLimits {
    /// Production project aggregation limits fixed by the contract.
    pub const PRODUCTION: Self = Self {
        diagnostics_per_document: 128,
        diagnostics_per_project: 512,
        work_per_project: 1_048_576,
    };

    pub const fn try_new(
        diagnostics_per_document: u16,
        diagnostics_per_project: u16,
        work_per_project: u64,
    ) -> Result<Self, NominalAggregationLimitsError> {
        if diagnostics_per_document == 0 {
            return Err(NominalAggregationLimitsError::Zero {
                kind: NominalAggregationLimitKind::DiagnosticsPerDocument,
            });
        }
        if diagnostics_per_project == 0 {
            return Err(NominalAggregationLimitsError::Zero {
                kind: NominalAggregationLimitKind::DiagnosticsPerProject,
            });
        }
        if work_per_project == 0 {
            return Err(NominalAggregationLimitsError::Zero {
                kind: NominalAggregationLimitKind::WorkPerProject,
            });
        }
        if diagnostics_per_document > Self::PRODUCTION.diagnostics_per_document {
            return Err(NominalAggregationLimitsError::AboveHardCeiling {
                kind: NominalAggregationLimitKind::DiagnosticsPerDocument,
                value: diagnostics_per_document as u64,
                ceiling: Self::PRODUCTION.diagnostics_per_document as u64,
            });
        }
        if diagnostics_per_project > Self::PRODUCTION.diagnostics_per_project {
            return Err(NominalAggregationLimitsError::AboveHardCeiling {
                kind: NominalAggregationLimitKind::DiagnosticsPerProject,
                value: diagnostics_per_project as u64,
                ceiling: Self::PRODUCTION.diagnostics_per_project as u64,
            });
        }
        if work_per_project > Self::PRODUCTION.work_per_project {
            return Err(NominalAggregationLimitsError::AboveHardCeiling {
                kind: NominalAggregationLimitKind::WorkPerProject,
                value: work_per_project,
                ceiling: Self::PRODUCTION.work_per_project,
            });
        }
        if diagnostics_per_document > diagnostics_per_project {
            return Err(
                NominalAggregationLimitsError::DocumentDiagnosticsExceedProject {
                    per_document: diagnostics_per_document,
                    per_project: diagnostics_per_project,
                },
            );
        }
        if work_per_project < diagnostics_per_project as u64 {
            return Err(NominalAggregationLimitsError::DiagnosticWorkInconsistent {
                diagnostics: diagnostics_per_project,
                work: work_per_project,
            });
        }
        Ok(Self {
            diagnostics_per_document,
            diagnostics_per_project,
            work_per_project,
        })
    }

    pub const fn diagnostics_per_document(self) -> u16 {
        self.diagnostics_per_document
    }

    pub const fn diagnostics_per_project(self) -> u16 {
        self.diagnostics_per_project
    }

    pub const fn work_per_project(self) -> u64 {
        self.work_per_project
    }
}

impl AcceptedNominalCatalogLimits {
    /// Production accepted-catalog limits fixed by the contract.
    pub const PRODUCTION: Self = Self {
        exact_records: 4_096,
        open_rules: 1_024,
    };

    pub const fn try_new(
        exact_records: u16,
        open_rules: u16,
    ) -> Result<Self, AcceptedNominalCatalogLimitsError> {
        if exact_records == 0 {
            return Err(AcceptedNominalCatalogLimitsError::Zero {
                kind: AcceptedNominalCatalogLimitKind::ExactRecords,
            });
        }
        if open_rules == 0 {
            return Err(AcceptedNominalCatalogLimitsError::Zero {
                kind: AcceptedNominalCatalogLimitKind::OpenRules,
            });
        }
        if exact_records > Self::PRODUCTION.exact_records {
            return Err(AcceptedNominalCatalogLimitsError::AboveHardCeiling {
                kind: AcceptedNominalCatalogLimitKind::ExactRecords,
                value: exact_records as u64,
                ceiling: Self::PRODUCTION.exact_records as u64,
            });
        }
        if open_rules > Self::PRODUCTION.open_rules {
            return Err(AcceptedNominalCatalogLimitsError::AboveHardCeiling {
                kind: AcceptedNominalCatalogLimitKind::OpenRules,
                value: open_rules as u64,
                ceiling: Self::PRODUCTION.open_rules as u64,
            });
        }
        Ok(Self {
            exact_records,
            open_rules,
        })
    }

    pub const fn exact_records(self) -> u16 {
        self.exact_records
    }

    pub const fn open_rules(self) -> u16 {
        self.open_rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_values_match_the_contract() {
        let resolution = NominalResolutionLimits::PRODUCTION;
        assert_eq!(resolution.type_nodes_per_reference(), 4_096);
        assert_eq!(resolution.recursive_type_depth(), 256);
        assert_eq!(resolution.generic_arguments_per_application(), 256);
        assert_eq!(resolution.alias_expansion_depth(), 64);
        assert_eq!(resolution.alias_expansion_nodes(), 16_384);
        assert_eq!(resolution.diagnostics_per_type_reference(), 32);
        assert_eq!(resolution.related_labels_per_diagnostic(), 32);
        assert_eq!(resolution.work_per_reference(), 65_536);

        let aggregation = NominalAggregationLimits::PRODUCTION;
        assert_eq!(aggregation.diagnostics_per_document(), 128);
        assert_eq!(aggregation.diagnostics_per_project(), 512);
        assert_eq!(aggregation.work_per_project(), 1_048_576);

        let catalog = AcceptedNominalCatalogLimits::PRODUCTION;
        assert_eq!(catalog.exact_records(), 4_096);
        assert_eq!(catalog.open_rules(), 1_024);
    }

    #[test]
    fn custom_limits_reject_zero_ceiling_and_inconsistent_work() {
        let production = NominalResolutionLimits::PRODUCTION;
        assert_eq!(
            NominalResolutionLimits::try_new(
                0,
                production.recursive_type_depth(),
                production.generic_arguments_per_application(),
                production.alias_expansion_depth(),
                production.alias_expansion_nodes(),
                production.diagnostics_per_type_reference(),
                production.related_labels_per_diagnostic(),
                production.work_per_reference(),
            ),
            Err(NominalResolutionLimitsError::Zero {
                kind: NominalResolutionLimitKind::TypeNodesPerReference,
            })
        );
        assert!(matches!(
            NominalResolutionLimits::try_new(4_097, 256, 256, 64, 16_384, 32, 32, 65_536),
            Err(NominalResolutionLimitsError::AboveHardCeiling {
                kind: NominalResolutionLimitKind::TypeNodesPerReference,
                ..
            })
        ));
        assert_eq!(
            NominalResolutionLimits::try_new(1, 1, 1, 1, 1, 2, 2, 5),
            Err(NominalResolutionLimitsError::DiagnosticWorkInconsistent {
                diagnostics: 2,
                related_labels: 2,
                work: 5,
            })
        );
    }

    #[test]
    fn aggregation_and_catalog_validate_their_invariants() {
        assert_eq!(
            NominalAggregationLimits::try_new(2, 1, 4),
            Err(
                NominalAggregationLimitsError::DocumentDiagnosticsExceedProject {
                    per_document: 2,
                    per_project: 1,
                }
            )
        );
        assert!(matches!(
            AcceptedNominalCatalogLimits::try_new(4_097, 1),
            Err(AcceptedNominalCatalogLimitsError::AboveHardCeiling {
                kind: AcceptedNominalCatalogLimitKind::ExactRecords,
                ..
            })
        ));
    }
}
