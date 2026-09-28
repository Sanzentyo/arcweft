use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatementCorpusDisposition {
    Accepted,
    Pending,
    RejectOnly,
    #[allow(
        dead_code,
        reason = "no live statement family has been proven unreachable"
    )]
    ProvenUnreachable,
}

// Each inventory row also generates an exhaustive match against its live
// HIR or checked owner enum. New variants must receive a disposition.
macro_rules! statement_family_inventory {
    ($family:ident for $owner:ty, { $($pattern:pat => $variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, StatementCorpusDisposition)] = &[
                $((Self::$variant, StatementCorpusDisposition::$disposition)),+
            ];

            fn of(owner: &$owner) -> Self {
                match owner {
                    $($pattern => Self::$variant),+
                }
            }
        }
    };
}

statement_family_inventory!(StatementShapeFamily for HirStmtKind, {
    HirStmtKind::Assertion { .. } => Assertion => Accepted,
    HirStmtKind::Let { .. } => Let => Accepted,
    HirStmtKind::Assign { .. } => Assign => Accepted,
    HirStmtKind::LetElse { .. } => LetElse => Accepted,
    HirStmtKind::Return { .. } => Return => Accepted,
    HirStmtKind::Out { .. } => Out => Accepted,
    HirStmtKind::Goto { .. } => Goto => Accepted,
    HirStmtKind::Defer { .. } => Defer => Accepted,
    HirStmtKind::Yield { .. } => Yield => Accepted,
    HirStmtKind::Signal { .. } => Signal => Pending,
    HirStmtKind::LifetimeSet { .. } => LifetimeSet => Pending,
    HirStmtKind::Wait { .. } => Wait => Accepted,
    HirStmtKind::On { .. } => On => Accepted,
    HirStmtKind::CancelRule { .. } => CancelRule => Accepted,
    HirStmtKind::UnsafeLifetime { .. } => UnsafeLifetime => Pending,
    HirStmtKind::Choice { .. } => Choice => Pending,
    HirStmtKind::If(_) => If => Accepted,
    HirStmtKind::IfLet(_) => IfLet => Accepted,
    HirStmtKind::Match(_) => Match => Pending,
    HirStmtKind::While(_) => While => Accepted,
    HirStmtKind::WhileLet(_) => WhileLet => Accepted,
    HirStmtKind::For(_) => For => Accepted,
    HirStmtKind::Close { .. } => Close => Pending,
    HirStmtKind::Select(_) => Select => Pending,
    HirStmtKind::SourceLocale(_) => SourceLocale => Accepted,
    HirStmtKind::Scope(_) => Scope => Accepted,
    HirStmtKind::Include(_) => Include => Accepted,
    HirStmtKind::Break { .. } => Break => Accepted,
    HirStmtKind::Continue { .. } => Continue => Accepted,
    HirStmtKind::Expression { .. } => Expression => Accepted,
    HirStmtKind::ProofCall { .. } => ProofCall => Pending,
    HirStmtKind::Error => Error => RejectOnly,
});

statement_family_inventory!(StatementPayloadFamily for CheckedStatementPayload, {
    CheckedStatementPayload::Structural => Structural => Accepted,
    CheckedStatementPayload::Assignment(_) => Assignment => Accepted,
    CheckedStatementPayload::Assertion(_) => Assertion => Accepted,
    CheckedStatementPayload::Defer(_) => Defer => Accepted,
    CheckedStatementPayload::EvaluatedEffect(_) => EvaluatedEffect => Pending,
    CheckedStatementPayload::Iteration(_) => Iteration => Accepted,
    CheckedStatementPayload::ControlTransfer(_) => ControlTransfer => Accepted,
    CheckedStatementPayload::Trigger(_) => Trigger => Accepted,
    CheckedStatementPayload::UnsafeAudit(_) => UnsafeAudit => Pending,
    CheckedStatementPayload::Select(_) => Select => Pending,
    CheckedStatementPayload::SourceLocale(_) => SourceLocale => Accepted,
    CheckedStatementPayload::Scope(_) => Scope => Accepted,
    CheckedStatementPayload::Include(_) => Include => Accepted,
    CheckedStatementPayload::Suspension(_) => Suspension => Accepted,
    CheckedStatementPayload::Yield => Yield => Accepted,
});

struct MatchStatementCorpusObservation {
    shapes: BTreeSet<StatementShapeFamily>,
    payloads: BTreeSet<StatementPayloadFamily>,
    statement_families: BTreeSet<(StatementShapeFamily, StatementPayloadFamily)>,
    assignments: Vec<CheckedAssignmentCorpusFact>,
    assertion_dispositions: Vec<CheckedAssertionDisposition>,
    defers: Vec<CheckedDeferCorpusFact>,
    iterations: Vec<CheckedIteration>,
    suspensions: Vec<CheckedSuspensionStatement>,
    triggers: Vec<super::super::super::CheckedTrigger>,
    source_locales: Vec<arcweft_id::LocaleTag>,
    scopes: Vec<super::super::super::CheckedScopeIdentity>,
    includes: Vec<super::super::super::CheckedIncludeFlowTarget>,
    loop_transfers: Vec<CheckedLoopTransferCorpusFact>,
    match_value_type: Option<crate::types::TypeKind>,
    semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedAssignmentCorpusFact {
    field_identity: [u8; 32],
    field_ordinal: u32,
    field_type: crate::types::TypeKind,
    value_type: crate::types::TypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedDeferCorpusFact {
    outcome: arcweft_lang_syntax::ast::line_plan::DeferOutcome,
    captures: Vec<(Vec<u8>, crate::types::TypeKind)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedLoopTransferCorpusFact {
    kind: arcweft_lang_hir::project::HirControlTransferKind,
    family: arcweft_lang_hir::project::HirLoopTargetFamily,
    body: Vec<u8>,
}

/// Count only checked statements owned below this accepted Match expression.
fn accepted_match_statement_corpus_observation(source: &str) -> MatchStatementCorpusObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("statement corpus source should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = match_owners.as_slice() else {
        panic!("each statement corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );
    let match_value_type = report
        .expression(*match_owner)
        .and_then(|expression| expression.value_type())
        .cloned();

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut shapes = BTreeSet::new();
    let mut payloads = BTreeSet::new();
    let mut statement_families = BTreeSet::new();
    let mut assignments = Vec::new();
    let mut assertion_dispositions = Vec::new();
    let mut defers = Vec::new();
    let mut iterations = Vec::new();
    let mut suspensions = Vec::new();
    let mut triggers = Vec::new();
    let mut source_locales = Vec::new();
    let mut scopes = Vec::new();
    let mut includes = Vec::new();
    let mut loop_transfers = Vec::new();
    for (owner, hir) in module.statements() {
        let Some(checked) = report.statement(owner) else {
            continue;
        };
        let coordinate = coordinates
            .statement(owner)
            .expect("checked statement owner path");
        if !coordinate.path().is_at_or_below(&match_path) {
            continue;
        }
        let shape = StatementShapeFamily::of(hir.kind());
        let payload = StatementPayloadFamily::of(checked.payload());
        shapes.insert(shape);
        payloads.insert(payload);
        statement_families.insert((shape, payload));
        match (hir.kind(), checked.payload()) {
            (HirStmtKind::Assign { .. }, CheckedStatementPayload::Assignment(assignment)) => {
                let field_place = assignment
                    .place()
                    .nominal_field()
                    .expect("Match assignment selects its checked nominal field");
                assignments.push(CheckedAssignmentCorpusFact {
                    field_identity: *field_place.field().field().as_bytes(),
                    field_ordinal: field_place.field().declaration_ordinal(),
                    field_type: field_place.field_type().clone(),
                    value_type: assignment.value_type().clone(),
                });
            }
            (HirStmtKind::Assertion { .. }, CheckedStatementPayload::Assertion(disposition)) => {
                assertion_dispositions.push(*disposition)
            }
            (HirStmtKind::Defer { .. }, CheckedStatementPayload::Defer(defer)) => {
                defers.push(CheckedDeferCorpusFact {
                    outcome: defer.outcome(),
                    captures: defer
                        .captures()
                        .iter()
                        .map(|capture| {
                            (
                                capture
                                    .origin()
                                    .canonical_bytes()
                                    .expect("defer capture has a stable checked origin"),
                                capture.ty().clone(),
                            )
                        })
                        .collect(),
                });
            }
            (HirStmtKind::For(_), CheckedStatementPayload::Iteration(iteration)) => {
                iterations.push(iteration.as_ref().clone())
            }
            (HirStmtKind::Wait { .. }, CheckedStatementPayload::Suspension(suspension)) => {
                suspensions.push(suspension.as_ref().clone())
            }
            (HirStmtKind::CancelRule { .. }, CheckedStatementPayload::Trigger(trigger)) => {
                triggers.push(trigger.clone());
            }
            (HirStmtKind::SourceLocale(_), CheckedStatementPayload::SourceLocale(locale)) => {
                source_locales.push(locale.clone());
            }
            (HirStmtKind::Scope(_), CheckedStatementPayload::Scope(scope)) => {
                scopes.push(scope.clone());
            }
            (HirStmtKind::Include(_), CheckedStatementPayload::Include(target)) => {
                includes.push(*target);
            }
            (
                HirStmtKind::Break { .. } | HirStmtKind::Continue { .. },
                CheckedStatementPayload::ControlTransfer(_),
            ) => {
                let evidence = coordinates
                    .control_transfer_evidence(owner)
                    .expect("accepted loop transfer has a checked target");
                assert_eq!(evidence.owner(), owner);
                let crate::semantic_coordinate::CheckedControlTransferTarget::Loop(target) =
                    evidence.target()
                else {
                    panic!("Break and Continue retain a checked loop target")
                };
                loop_transfers.push(CheckedLoopTransferCorpusFact {
                    kind: evidence.kind(),
                    family: target.family(),
                    body: target
                        .body()
                        .canonical_bytes()
                        .expect("checked loop target has a stable body coordinate"),
                });
            }
            (HirStmtKind::Break { .. } | HirStmtKind::Continue { .. }, _) => {
                panic!("accepted loop transfer has its exact ControlTransfer payload")
            }
            (HirStmtKind::Assign { .. }, _) => {
                panic!("accepted Assign has its exact checked Assignment payload")
            }
            (HirStmtKind::Assertion { .. }, _) => {
                panic!("accepted Assertion has its exact checked Assertion payload")
            }
            (HirStmtKind::Defer { .. }, _) => {
                panic!("accepted Defer has its exact checked Defer payload")
            }
            (HirStmtKind::For(_), _) => {
                panic!("accepted For has its exact checked Iteration payload")
            }
            (HirStmtKind::Wait { .. }, _) => {
                panic!("accepted Wait has its exact checked Suspension payload")
            }
            (HirStmtKind::CancelRule { .. }, _) => {
                panic!("accepted CancelRule has its exact checked Trigger payload")
            }
            (HirStmtKind::SourceLocale(_), _) => {
                panic!("accepted SourceLocale has its exact checked SourceLocale payload")
            }
            (HirStmtKind::Scope(_), _) => {
                panic!("accepted Scope has its exact checked Scope payload")
            }
            (HirStmtKind::Include(_), _) => {
                panic!("accepted Include has its exact checked Include payload")
            }
            _ => {}
        }
    }
    assert!(
        !shapes.is_empty(),
        "Match root has checked statement descendants"
    );
    MatchStatementCorpusObservation {
        shapes,
        payloads,
        statement_families,
        assignments,
        assertion_dispositions,
        defers,
        iterations,
        suspensions,
        triggers,
        source_locales,
        scopes,
        includes,
        loop_transfers,
        match_value_type,
        semantic_digest: *product.semantic_digest().as_bytes(),
    }
}

fn match_arm_assignment_source(field: &str) -> String {
    format!(
        "struct Flags {{ left: bool, right: bool }}\n\
         fn root(flags: Flags, flag: bool) -> bool {{\n\
             match flag {{\n\
                 true => {{\n\
                     flags.{field} = flag\n\
                     true\n\
                 }}\n\
                 false => false\n\
             }}\n\
         }}\n"
    )
}

fn match_arm_assertion_source(operand: &str) -> String {
    format!(
        "fn root(flag: bool, other: bool) -> i64 {{\n\
             match flag {{\n\
                 true => {{\n\
                     assert.check({operand})\n\
                     1i64\n\
                 }}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
    )
}

fn function_match_statement_source(statement: &str) -> String {
    let statement = statement
        .lines()
        .map(|line| format!("            {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"fn root(flag: bool, other: bool) -> i64 {{
    match flag {{
        true => {{
{statement}
            1i64
        }}
        false => 0i64
    }}
}}
"#
    )
}

fn flow_match_statement_source(statement: &str) -> String {
    let statement = statement
        .lines()
        .map(|line| format!("                     {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "flow root(flag: bool, other: bool) {{\n\
             let selected = match flag {{\n\
                 true => {{\n\
{statement}\n\
                     1i64\n\
                 }}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
    )
}

fn flow_match_defer_source(capture: &str) -> String {
    flow_match_statement_source(&format!("defer {{\n    let captured = {capture};\n}}"))
}

fn flow_match_thread_source(statement: &str) -> String {
    flow_match_statement_source(&format!("thread {{ {statement} }}"))
}

fn flow_match_include_source(target: &str) -> String {
    format!(
        "flow shared() {{}}\nflow alternate() {{}}\n{}",
        flow_match_thread_source(&format!("include @flow.{target}")),
    )
}

fn flow_match_goto_source(target: &str) -> String {
    format!(
        "flow done {{}}\nflow alternate {{}}\n{}",
        flow_match_statement_source(&format!("goto @flow.{target}")),
    )
}

fn dialogue_match_wait_source(wait: &str) -> String {
    format!(
        r#"pub character alice {{ display = "Alice" }}
flow row() -> Unit {{
    let selected = match true {{
        true => {{
            alice[before [mark @.release] after[p]] with {{
                {wait}
                on mark(@.release) {{ out "Released" }}
            }}
            "Released"
        }}
        false => "Moved"
    }}
    return ()
}}
"#
    )
}

fn dialogue_match_cancel_source(cancel: &str) -> String {
    format!(
        r#"pub character alice {{ display = "Alice" }}
flow row() -> Unit {{
    let selected = match true {{
        true => {{
            alice[before [mark @.release] after[p]] with {{
                {cancel}
                on mark(@.release) {{ out "Released" }}
            }}
            "Released"
        }}
        false => "Moved"
    }}
    return ()
}}
"#
    )
}

fn assert_statement_inventory<T: Copy + Ord + std::fmt::Debug>(
    axis: &str,
    observed: &BTreeSet<T>,
    inventory: &[(T, StatementCorpusDisposition)],
) {
    for (family, disposition) in inventory {
        assert_eq!(
            observed.contains(family),
            *disposition == StatementCorpusDisposition::Accepted,
            "{axis} {family:?} corpus disposition {disposition:?}",
        );
    }
}

#[test]
fn checked_match_statement_corpus_tracks_accepted_root_families() {
    struct Row {
        name: &'static str,
        source: String,
        shapes: &'static [StatementShapeFamily],
        payloads: &'static [StatementPayloadFamily],
    }
    let rows = [
        Row {
            name: "arm block binding",
            source: match_arm_let_source("1i64"),
            shapes: &[StatementShapeFamily::Let],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "nested If and binding",
            source: nested_if_statement_source("                let first = 1i64"),
            shapes: &[StatementShapeFamily::If, StatementShapeFamily::Let],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "nominal field assignment in arm block",
            source: match_arm_assignment_source("left"),
            shapes: &[StatementShapeFamily::Assign],
            payloads: &[StatementPayloadFamily::Assignment],
        },
        Row {
            name: "runtime assertion in arm block",
            source: match_arm_assertion_source("flag"),
            shapes: &[StatementShapeFamily::Assertion],
            payloads: &[StatementPayloadFamily::Assertion],
        },
        Row {
            name: "Return in arm block",
            source: function_match_statement_source("return 1i64"),
            shapes: &[StatementShapeFamily::Return],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "Goto in Flow Match arm block",
            source: flow_match_goto_source("done"),
            shapes: &[StatementShapeFamily::Goto],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "Yield in Flow Match arm block",
            source: flow_match_statement_source("yield flag"),
            shapes: &[StatementShapeFamily::Yield],
            payloads: &[StatementPayloadFamily::Yield],
        },
        Row {
            name: "Break in Flow Match arm loop",
            source: flow_match_statement_source("loop { break }"),
            shapes: &[StatementShapeFamily::Break],
            payloads: &[StatementPayloadFamily::ControlTransfer],
        },
        Row {
            name: "Continue in Flow Match arm loop",
            source: flow_match_statement_source("loop { continue }"),
            shapes: &[StatementShapeFamily::Continue],
            payloads: &[StatementPayloadFamily::ControlTransfer],
        },
        Row {
            name: "IfLet in arm block",
            source: function_match_statement_source("if let value = other {}"),
            shapes: &[StatementShapeFamily::IfLet],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "LetElse and failure Return in arm block",
            source: function_match_statement_source("let value = true else { return 3i64 }"),
            shapes: &[StatementShapeFamily::LetElse, StatementShapeFamily::Return],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "Defer in Flow arm block",
            source: flow_match_defer_source("flag"),
            shapes: &[StatementShapeFamily::Defer, StatementShapeFamily::Let],
            payloads: &[
                StatementPayloadFamily::Defer,
                StatementPayloadFamily::Structural,
            ],
        },
        Row {
            name: "For in Flow arm block",
            source: flow_match_statement_source("for value in [true, false] {}"),
            shapes: &[StatementShapeFamily::For],
            payloads: &[StatementPayloadFamily::Iteration],
        },
        Row {
            name: "Thread While in arm block",
            source: flow_match_thread_source("while other {}"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::While,
            ],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "Thread WhileLet in arm block",
            source: flow_match_thread_source("while let value = other {}"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::WhileLet,
            ],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "Wait in dialogue line-plan Match arm block",
            source: dialogue_match_wait_source("wait(1s)"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::On,
                StatementShapeFamily::Out,
                StatementShapeFamily::Wait,
            ],
            payloads: &[
                StatementPayloadFamily::ControlTransfer,
                StatementPayloadFamily::Structural,
                StatementPayloadFamily::Suspension,
                StatementPayloadFamily::Trigger,
            ],
        },
        Row {
            name: "CancelRule in dialogue line-plan Match arm block",
            source: dialogue_match_cancel_source("cancel on input(.SkipLine) { out \"Skipped\" }"),
            shapes: &[
                StatementShapeFamily::CancelRule,
                StatementShapeFamily::Expression,
                StatementShapeFamily::On,
                StatementShapeFamily::Out,
            ],
            payloads: &[
                StatementPayloadFamily::ControlTransfer,
                StatementPayloadFamily::Structural,
                StatementPayloadFamily::Trigger,
            ],
        },
        Row {
            name: "SourceLocale in nested Thread arm block",
            source: flow_match_thread_source("source locale en-US {}"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::SourceLocale,
            ],
            payloads: &[
                StatementPayloadFamily::SourceLocale,
                StatementPayloadFamily::Structural,
            ],
        },
        Row {
            name: "Scope in nested Thread arm block",
            source: flow_match_thread_source("scope local {}"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::Scope,
            ],
            payloads: &[
                StatementPayloadFamily::Scope,
                StatementPayloadFamily::Structural,
            ],
        },
        Row {
            name: "Include in nested Thread arm block",
            source: flow_match_include_source("shared"),
            shapes: &[
                StatementShapeFamily::Expression,
                StatementShapeFamily::Include,
            ],
            payloads: &[
                StatementPayloadFamily::Include,
                StatementPayloadFamily::Structural,
            ],
        },
    ];
    let mut shapes = BTreeSet::new();
    let mut payloads = BTreeSet::new();
    for row in rows {
        let observation = accepted_match_statement_corpus_observation(&row.source);
        for required in row.shapes {
            assert!(
                observation.shapes.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        for required in row.payloads {
            assert!(
                observation.payloads.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        shapes.extend(observation.shapes);
        payloads.extend(observation.payloads);
    }
    assert_statement_inventory("HIR statement", &shapes, StatementShapeFamily::INVENTORY);
    assert_statement_inventory(
        "checked statement",
        &payloads,
        StatementPayloadFamily::INVENTORY,
    );
}

#[test]
fn checked_match_flow_control_bundle_retains_exact_payloads_and_meaning() {
    struct Row {
        name: &'static str,
        source: String,
        expected: BTreeSet<(StatementShapeFamily, StatementPayloadFamily)>,
    }

    let rows = [
        Row {
            name: "Goto target",
            source: flow_match_goto_source("done"),
            expected: BTreeSet::from([(
                StatementShapeFamily::Goto,
                StatementPayloadFamily::Structural,
            )]),
        },
        Row {
            name: "alternate Goto target",
            source: flow_match_goto_source("alternate"),
            expected: BTreeSet::from([(
                StatementShapeFamily::Goto,
                StatementPayloadFamily::Structural,
            )]),
        },
        Row {
            name: "Yield flag operand",
            source: flow_match_statement_source("yield flag"),
            expected: BTreeSet::from([(
                StatementShapeFamily::Yield,
                StatementPayloadFamily::Yield,
            )]),
        },
        Row {
            name: "Yield other operand",
            source: flow_match_statement_source("yield other"),
            expected: BTreeSet::from([(
                StatementShapeFamily::Yield,
                StatementPayloadFamily::Yield,
            )]),
        },
        Row {
            name: "Break loop target",
            source: flow_match_statement_source("loop { break }"),
            expected: BTreeSet::from([
                (
                    StatementShapeFamily::Break,
                    StatementPayloadFamily::ControlTransfer,
                ),
                (
                    StatementShapeFamily::Expression,
                    StatementPayloadFamily::Structural,
                ),
            ]),
        },
        Row {
            name: "Continue loop target",
            source: flow_match_statement_source("loop { continue }"),
            expected: BTreeSet::from([
                (
                    StatementShapeFamily::Continue,
                    StatementPayloadFamily::ControlTransfer,
                ),
                (
                    StatementShapeFamily::Expression,
                    StatementPayloadFamily::Structural,
                ),
            ]),
        },
    ];
    let observations = rows
        .iter()
        .map(|row| {
            let observation = accepted_match_statement_corpus_observation(&row.source);
            assert_eq!(
                observation.statement_families, row.expected,
                "{} retains its exact HIR shape and checked payload",
                row.name,
            );
            observation
        })
        .collect::<Vec<_>>();

    let goto = &observations[0];
    let alternate_goto = &observations[1];
    assert_ne!(
        goto.semantic_digest, alternate_goto.semantic_digest,
        "the resolved Flow target contributes to the Match digest",
    );

    let yield_flag = &observations[2];
    let yield_other = &observations[3];
    assert_ne!(
        yield_flag.semantic_digest, yield_other.semantic_digest,
        "the checked Yield operand contributes through its expression child edge",
    );

    let break_transfer = &observations[4];
    let continue_transfer = &observations[5];
    let [break_fact] = break_transfer.loop_transfers.as_slice() else {
        panic!("the Match descendant retains one checked Break target")
    };
    let [continue_fact] = continue_transfer.loop_transfers.as_slice() else {
        panic!("the Match descendant retains one checked Continue target")
    };
    assert_eq!(
        break_fact.kind,
        arcweft_lang_hir::project::HirControlTransferKind::Break
    );
    assert_eq!(
        continue_fact.kind,
        arcweft_lang_hir::project::HirControlTransferKind::Continue
    );
    assert_eq!(
        break_fact.family,
        arcweft_lang_hir::project::HirLoopTargetFamily::LoopExpression
    );
    assert_eq!(break_fact.family, continue_fact.family);
    assert_eq!(break_fact.body, continue_fact.body);
    assert_ne!(
        break_transfer.semantic_digest, continue_transfer.semantic_digest,
        "the transfer operation contributes to the Match digest",
    );
}

#[test]
fn checked_match_assignments_retain_same_typed_field_identity_and_digest() {
    let left = accepted_match_statement_corpus_observation(&match_arm_assignment_source("left"));
    let right = accepted_match_statement_corpus_observation(&match_arm_assignment_source("right"));

    assert_eq!(left.shapes, BTreeSet::from([StatementShapeFamily::Assign]));
    assert_eq!(
        left.payloads,
        BTreeSet::from([StatementPayloadFamily::Assignment])
    );
    assert_eq!(left.assignments.len(), 1);
    assert_eq!(right.assignments.len(), 1);
    assert_eq!(left.assignments[0].field_ordinal, 0);
    assert_eq!(right.assignments[0].field_ordinal, 1);
    assert_ne!(
        left.assignments[0].field_identity,
        right.assignments[0].field_identity
    );
    assert_eq!(left.assignments[0].field_type, crate::types::TypeKind::Bool);
    assert_eq!(
        right.assignments[0].field_type,
        crate::types::TypeKind::Bool
    );
    assert_eq!(left.assignments[0].value_type, crate::types::TypeKind::Bool);
    assert_eq!(
        right.assignments[0].value_type,
        crate::types::TypeKind::Bool
    );
    assert_ne!(left.semantic_digest, right.semantic_digest);
}

#[test]
fn checked_match_assertions_retain_disposition_and_condition_meaning() {
    let flag = accepted_match_statement_corpus_observation(&match_arm_assertion_source("flag"));
    let other = accepted_match_statement_corpus_observation(&match_arm_assertion_source("other"));
    let expected =
        CheckedAssertionDisposition::Runtime(crate::assertion::AssertionRuntimePolicy::AlwaysGuard);

    assert_eq!(
        flag.shapes,
        BTreeSet::from([StatementShapeFamily::Assertion])
    );
    assert_eq!(
        flag.payloads,
        BTreeSet::from([StatementPayloadFamily::Assertion])
    );
    assert_eq!(flag.assertion_dispositions, [expected]);
    assert_eq!(other.assertion_dispositions, [expected]);
    assert_ne!(flag.semantic_digest, other.semantic_digest);
}

#[test]
fn checked_match_defer_retains_capture_meaning() {
    let flag = accepted_match_statement_corpus_observation(&flow_match_defer_source("flag"));
    let other = accepted_match_statement_corpus_observation(&flow_match_defer_source("other"));

    assert_eq!(
        flag.shapes,
        BTreeSet::from([StatementShapeFamily::Defer, StatementShapeFamily::Let])
    );
    assert_eq!(
        flag.payloads,
        BTreeSet::from([
            StatementPayloadFamily::Defer,
            StatementPayloadFamily::Structural,
        ])
    );
    assert_eq!(flag.defers.len(), 1);
    assert_eq!(other.defers.len(), 1);
    assert_eq!(
        flag.defers[0].outcome,
        arcweft_lang_syntax::ast::line_plan::DeferOutcome::Always
    );
    assert_eq!(flag.defers[0].captures.len(), 1);
    assert_eq!(other.defers[0].captures.len(), 1);
    assert_eq!(flag.defers[0].captures[0].1, crate::types::TypeKind::Bool);
    assert_eq!(other.defers[0].captures[0].1, crate::types::TypeKind::Bool);
    assert_ne!(flag.defers[0].captures[0].0, other.defers[0].captures[0].0);
    assert_ne!(flag.semantic_digest, other.semantic_digest);
}

#[test]
fn checked_match_for_retains_builtin_iteration_and_iterable_meaning() {
    let ordered = accepted_match_statement_corpus_observation(&flow_match_statement_source(
        "for value in [true, false] {}",
    ));
    let repeated = accepted_match_statement_corpus_observation(&flow_match_statement_source(
        "for value in [true, true] {}",
    ));

    assert_eq!(ordered.shapes, BTreeSet::from([StatementShapeFamily::For]));
    assert_eq!(
        ordered.payloads,
        BTreeSet::from([StatementPayloadFamily::Iteration])
    );
    for observation in [&ordered, &repeated] {
        assert!(
            matches!(
                observation.iterations.as_slice(),
                [CheckedIteration::Builtin {
                    family: CheckedIteratorFamily::Vec,
                    item,
                }] if item == &crate::types::TypeKind::Bool
            ),
            "observed checked iteration: {:?}",
            observation.iterations
        );
    }
    assert_ne!(ordered.semantic_digest, repeated.semantic_digest);
}

#[test]
fn checked_match_return_retains_exact_shape_and_meaning() {
    let return_one = accepted_match_statement_corpus_observation(&function_match_statement_source(
        "return 1i64",
    ));
    let return_two = accepted_match_statement_corpus_observation(&function_match_statement_source(
        "return 2i64",
    ));

    let expected = BTreeSet::from([(
        StatementShapeFamily::Return,
        StatementPayloadFamily::Structural,
    )]);
    assert_eq!(return_one.statement_families, expected);
    assert_eq!(return_two.statement_families, expected);
    assert_ne!(return_one.semantic_digest, return_two.semantic_digest);
}

#[test]
fn checked_match_if_let_retains_exact_shape_and_input_meaning() {
    let other = accepted_match_statement_corpus_observation(&function_match_statement_source(
        "if let value = other {}",
    ));
    let flag = accepted_match_statement_corpus_observation(&function_match_statement_source(
        "if let value = flag {}",
    ));

    let expected = BTreeSet::from([(
        StatementShapeFamily::IfLet,
        StatementPayloadFamily::Structural,
    )]);
    assert_eq!(other.statement_families, expected);
    assert_eq!(flag.statement_families, expected);
    assert_ne!(other.semantic_digest, flag.semantic_digest);
}

#[test]
fn checked_match_let_else_retains_exact_shapes_and_initializer_meaning() {
    let true_initializer = accepted_match_statement_corpus_observation(
        &function_match_statement_source("let value = true else { return 3i64 }"),
    );
    let false_initializer = accepted_match_statement_corpus_observation(
        &function_match_statement_source("let value = false else { return 3i64 }"),
    );
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::LetElse,
            StatementPayloadFamily::Structural,
        ),
        (
            StatementShapeFamily::Return,
            StatementPayloadFamily::Structural,
        ),
    ]);

    assert_eq!(true_initializer.statement_families, expected);
    assert_eq!(false_initializer.statement_families, expected);
    assert_ne!(
        true_initializer.semantic_digest,
        false_initializer.semantic_digest
    );
}

#[test]
fn checked_match_thread_while_candidate_reaches_checked_statement_path() {
    let other =
        accepted_match_statement_corpus_observation(&flow_match_thread_source("while other {}"));
    let flag =
        accepted_match_statement_corpus_observation(&flow_match_thread_source("while flag {}"));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (
            StatementShapeFamily::While,
            StatementPayloadFamily::Structural,
        ),
    ]);

    assert_eq!(other.statement_families, expected);
    assert_eq!(flag.statement_families, expected);
    assert_ne!(other.semantic_digest, flag.semantic_digest);
}

#[test]
fn checked_match_thread_while_let_candidate_reaches_checked_statement_path() {
    let other = accepted_match_statement_corpus_observation(&flow_match_thread_source(
        "while let value = other {}",
    ));
    let flag = accepted_match_statement_corpus_observation(&flow_match_thread_source(
        "while let value = flag {}",
    ));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (
            StatementShapeFamily::WhileLet,
            StatementPayloadFamily::Structural,
        ),
    ]);

    assert_eq!(other.statement_families, expected);
    assert_eq!(flag.statement_families, expected);
    assert_ne!(other.semantic_digest, flag.semantic_digest);
}

#[test]
fn checked_match_dialogue_wait_reaches_checked_statement_path_and_changes_digest() {
    let one_second =
        accepted_match_statement_corpus_observation(&dialogue_match_wait_source("wait(1s)"));
    let two_seconds =
        accepted_match_statement_corpus_observation(&dialogue_match_wait_source("wait(2s)"));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (StatementShapeFamily::On, StatementPayloadFamily::Trigger),
        (
            StatementShapeFamily::Out,
            StatementPayloadFamily::ControlTransfer,
        ),
        (
            StatementShapeFamily::Wait,
            StatementPayloadFamily::Suspension,
        ),
    ]);

    assert_eq!(one_second.statement_families, expected);
    assert_eq!(two_seconds.statement_families, expected);
    assert_eq!(
        one_second.match_value_type,
        Some(crate::types::TypeKind::String)
    );
    assert_eq!(
        two_seconds.match_value_type,
        Some(crate::types::TypeKind::String)
    );
    assert_eq!(one_second.suspensions, [CheckedSuspensionStatement::Wait]);
    assert_eq!(two_seconds.suspensions, [CheckedSuspensionStatement::Wait]);
    assert_ne!(one_second.semantic_digest, two_seconds.semantic_digest);
}

#[test]
fn checked_match_dialogue_cancel_rule_reaches_exact_trigger_payload() {
    let absent = accepted_match_statement_corpus_observation(&dialogue_match_cancel_source(""));
    let skip = accepted_match_statement_corpus_observation(&dialogue_match_cancel_source(
        "cancel on input(.SkipLine) { out \"Skipped\" }",
    ));
    let back = accepted_match_statement_corpus_observation(&dialogue_match_cancel_source(
        "cancel on input(.BackToTitle) { out \"Skipped\" }",
    ));
    let absent_families = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (StatementShapeFamily::On, StatementPayloadFamily::Trigger),
        (
            StatementShapeFamily::Out,
            StatementPayloadFamily::ControlTransfer,
        ),
    ]);
    let cancel_families = BTreeSet::from([
        (
            StatementShapeFamily::CancelRule,
            StatementPayloadFamily::Trigger,
        ),
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (StatementShapeFamily::On, StatementPayloadFamily::Trigger),
        (
            StatementShapeFamily::Out,
            StatementPayloadFamily::ControlTransfer,
        ),
    ]);

    assert_eq!(absent.statement_families, absent_families);
    assert_eq!(skip.statement_families, cancel_families);
    assert_eq!(back.statement_families, cancel_families);
    assert!(absent.triggers.is_empty());
    assert!(matches!(
        skip.triggers.as_slice(),
        [trigger]
            if matches!(
                trigger.view(),
                super::super::super::CheckedTriggerView::InputAction(action)
                    if action.as_str() == "SkipLine"
            )
    ));
    assert!(matches!(
        back.triggers.as_slice(),
        [trigger]
            if matches!(
                trigger.view(),
                super::super::super::CheckedTriggerView::InputAction(action)
                    if action.as_str() == "BackToTitle"
            )
    ));
    assert_eq!(
        absent.match_value_type,
        Some(crate::types::TypeKind::String)
    );
    assert_eq!(skip.match_value_type, Some(crate::types::TypeKind::String));
    assert_eq!(back.match_value_type, Some(crate::types::TypeKind::String));
    assert_ne!(absent.semantic_digest, skip.semantic_digest);
    assert_ne!(skip.semantic_digest, back.semantic_digest);
}

#[test]
fn checked_match_thread_source_locale_reaches_exact_payload() {
    let english = accepted_match_statement_corpus_observation(&flow_match_thread_source(
        "source locale en-US {}",
    ));
    let japanese = accepted_match_statement_corpus_observation(&flow_match_thread_source(
        "source locale ja-JP {}",
    ));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (
            StatementShapeFamily::SourceLocale,
            StatementPayloadFamily::SourceLocale,
        ),
    ]);

    assert_eq!(english.statement_families, expected);
    assert_eq!(japanese.statement_families, expected);
    assert_eq!(english.source_locales.len(), 1);
    assert_eq!(japanese.source_locales.len(), 1);
    assert_ne!(english.source_locales, japanese.source_locales);
    assert_ne!(english.semantic_digest, japanese.semantic_digest);
}

#[test]
fn checked_match_thread_scope_retains_accepted_name_and_format_invariance() {
    let local =
        accepted_match_statement_corpus_observation(&flow_match_thread_source("scope local {}"));
    let scene =
        accepted_match_statement_corpus_observation(&flow_match_thread_source("scope scene {}"));
    let formatted =
        accepted_match_statement_corpus_observation(&flow_match_thread_source("scope local { }"));
    let anonymous = accepted_match_statement_corpus_observation(&flow_match_thread_source("{}"));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (StatementShapeFamily::Scope, StatementPayloadFamily::Scope),
    ]);

    for observation in [&local, &scene, &formatted, &anonymous] {
        assert_eq!(observation.statement_families, expected);
        assert_eq!(observation.scopes.len(), 1);
    }
    assert!(matches!(
        local.scopes.as_slice(),
        [super::super::super::CheckedScopeIdentity::Named(name)] if name.as_str() == "local"
    ));
    assert!(matches!(
        scene.scopes.as_slice(),
        [super::super::super::CheckedScopeIdentity::Named(name)] if name.as_str() == "scene"
    ));
    assert_eq!(local.scopes, formatted.scopes);
    assert_eq!(
        anonymous.scopes,
        [super::super::super::CheckedScopeIdentity::Anonymous]
    );
    assert_ne!(local.semantic_digest, scene.semantic_digest);
    assert_ne!(local.semantic_digest, anonymous.semantic_digest);
    assert_eq!(local.semantic_digest, formatted.semantic_digest);
}

#[test]
fn checked_match_thread_include_reaches_exact_flow_target() {
    let shared = accepted_match_statement_corpus_observation(&flow_match_include_source("shared"));
    let alternate =
        accepted_match_statement_corpus_observation(&flow_match_include_source("alternate"));
    let expected = BTreeSet::from([
        (
            StatementShapeFamily::Expression,
            StatementPayloadFamily::Structural,
        ),
        (
            StatementShapeFamily::Include,
            StatementPayloadFamily::Include,
        ),
    ]);

    assert_eq!(shared.statement_families, expected);
    assert_eq!(alternate.statement_families, expected);
    assert_eq!(shared.includes.len(), 1);
    assert_eq!(alternate.includes.len(), 1);
    assert_ne!(
        shared.includes[0].declaration(),
        alternate.includes[0].declaration()
    );
    assert_ne!(shared.semantic_digest, alternate.semantic_digest);
}

#[test]
fn checked_match_statement_corpus_excludes_unrelated_declaration_statements() {
    let source = format!(
        "fn unrelated() -> i64 {{ return 9i64 }}\n{}",
        match_arm_let_source("1i64"),
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated Return should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.statements().any(|(owner, statement)| {
        matches!(statement.kind(), HirStmtKind::Return { .. }) && report.statement(owner).is_some()
    }));
    let observation = accepted_match_statement_corpus_observation(&source);
    assert!(observation.shapes.contains(&StatementShapeFamily::Let));
    assert!(
        !observation.shapes.contains(&StatementShapeFamily::Return),
        "an unrelated checked Return must not enter the Match corpus",
    );
}
