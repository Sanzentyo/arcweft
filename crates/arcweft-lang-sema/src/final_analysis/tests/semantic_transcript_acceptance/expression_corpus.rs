use super::*;

#[path = "contextual_expressions.rs"]
mod contextual_expressions;

fn entry_reference_match_source(first: &str, second: &str) -> String {
    format!(
        "flow done() -> String {{ return \"done\" }}\nflow @flow.references references {{\n    let selected = match true {{\n        true => {first}\n        false => {second}\n    }}\n}}\nentry cli @entry.cli.primary {{ goto @flow.done }}\nentry cli @entry.cli.alternate {{ goto @flow.done }}\n"
    )
}

fn project_item_match_source(first: &str, second: &str) -> String {
    format!(
        "pub character alternate {{}}\nfn root(flag: bool) -> Ref<Character> {{\n    match flag {{\n        true => {first}\n        false => {second}\n    }}\n}}\n"
    )
}

fn registered_value_match_source(binding: &str) -> String {
    format!(
        "fn root(flag: bool) -> i32 {{\n    match flag {{\n        true => {binding}\n        false => 0i32\n    }}\n}}\n"
    )
}

fn range_match_source(inclusive: bool) -> String {
    let right = if inclusive { "..=" } else { ".." };
    format!(
        "fn root(flag: bool) -> i64 {{\n    let selected = match flag {{\n        true => 0i64{right}1i64\n        false => 0i64..2i64\n    }}\n    0i64\n}}\n"
    )
}

fn bracket_sequence_match_source(left: &str, right: &str) -> String {
    format!(
        "fn root(flag: bool) -> Array<bool, 2> {{\n    match flag {{\n        true => [{left}]\n        false => [{right}]\n    }}\n}}\n"
    )
}

fn array_repeat_match_source(value: &str) -> String {
    format!(
        "fn root(flag: bool) -> Array<i32, 2> {{\n    match flag {{\n        true => [{value}i32; 2i64]\n        false => [9i32; 2i64]\n    }}\n}}\n"
    )
}

fn index_match_source(index: usize) -> String {
    format!(
        "fn root(items: Vec<i64>, flag: bool) -> i64 {{\n    match flag {{\n        true => items[{index}]\n        false => 0i64\n    }}\n}}\n"
    )
}

fn progress_field_match_source(field: &str) -> String {
    format!(
        "fn observe(need: Need<i64>) -> i64 {{\n    await need with {{\n        pending progress => {{\n            let selected = match true {{\n                true => progress.{field}\n                false => progress.{field}\n            }}\n        }}\n    }}\n}}\n"
    )
}

fn dialogue_line_context_match_source() -> String {
    concat!(
        "pub character akane {}\n",
        "flow line_context_match() -> String {\n",
        "    let (_, cue) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        at(0.20s):\n",
        "            actor.look(.normal)\n",
        "        let later_actor = akane.stage.acquire(scope=line)\n",
        "        let cue = at(0.42s):\n",
        "            later_actor.look(.normal, crossfade=120ms)\n",
        "        let voice = match true {\n",
        "            true => line.voice_handle()\n",
        "            false => line.voice_handle()\n",
        "        }\n",
        "        out (voice, cue)\n",
        "    return \"done\"\n",
        "}\n",
    )
    .to_owned()
}

fn dialogue_character_field_match_source(character: &str) -> String {
    format!(
        concat!(
            "pub character akane {{}}\n",
            "pub character alternate {{}}\n",
            "flow character_field_match() -> String {{\n",
            "    let (retained, _) = akane(voice=auto)[聞いて。[p]]\n",
            "    with:\n",
            "        let actor = match true {{\n",
            "            true => {character}.stage.acquire(scope=line)\n",
            "            false => {character}.stage.acquire(scope=line)\n",
            "        }}\n",
            "        out (actor, ())\n",
            "    return \"done\"\n",
            "}}\n",
        ),
        character = character,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionCorpusDisposition {
    Accepted,
    RejectOnly,
    ProvenUnreachable,
}

// Each list generates both the corpus disposition and an exhaustive classifier
// against its live HIR or checked owner enum.
macro_rules! expression_family_inventory {
    ($family:ident for $owner:ty, { $($pattern:pat => $variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, ExpressionCorpusDisposition)] = &[
                $((Self::$variant, ExpressionCorpusDisposition::$disposition)),+
            ];

            fn of(owner: &$owner) -> Self {
                match owner {
                    $($pattern => Self::$variant),+
                }
            }
        }
    };
}

expression_family_inventory!(ExprShapeFamily for HirExprKind, {
    HirExprKind::Unit => Unit => Accepted,
    HirExprKind::Literal(_) => Literal => Accepted,
    HirExprKind::EntityReference(_) => EntityReference => Accepted,
    HirExprKind::LifetimePath(_) => LifetimePath => RejectOnly,
    HirExprKind::Path(_) => Path => Accepted,
    HirExprKind::ShortVariant(_) => ShortVariant => Accepted,
    HirExprKind::Placeholder(_) => Placeholder => Accepted,
    HirExprKind::Tuple(_) => Tuple => Accepted,
    HirExprKind::BracketSequence(_) => BracketSequence => Accepted,
    HirExprKind::NumericBracketSequence(_) => NumericBracketSequence => Accepted,
    HirExprKind::ArrayRepeat(_) => ArrayRepeat => Accepted,
    HirExprKind::Call(_) => Call => Accepted,
    HirExprKind::Select(_) => Select => Accepted,
    HirExprKind::Index(_) => Index => Accepted,
    HirExprKind::Pipe(_) => Pipe => Accepted,
    HirExprKind::Try(_) => Try => Accepted,
    HirExprKind::Await(_) => Await => Accepted,
    HirExprKind::Thread(_) => Thread => Accepted,
    HirExprKind::Choice(_) => Choice => Accepted,
    HirExprKind::Range(_) => Range => Accepted,
    HirExprKind::Record(_) => Record => Accepted,
    HirExprKind::RecordLiteral(_) => RecordLiteral => Accepted,
    HirExprKind::Binary(_) => Binary => Accepted,
    HirExprKind::Borrow(_) => Borrow => Accepted,
    HirExprKind::Dereference(_) => Dereference => Accepted,
    HirExprKind::Closure(_) => Closure => Accepted,
    HirExprKind::Unary(_) => Unary => Accepted,
    HirExprKind::Block(_) => Block => Accepted,
    HirExprKind::ComputationBlock(_) => ComputationBlock => Accepted,
    HirExprKind::NamedBlock(_) => NamedBlock => Accepted,
    HirExprKind::Loop(_) => Loop => Accepted,
    HirExprKind::If(_) => If => Accepted,
    HirExprKind::IfLet(_) => IfLet => Accepted,
    HirExprKind::Match(_) => Match => Accepted,
    HirExprKind::AttachedContentApplication(_) => AttachedContentApplication => Accepted,
    HirExprKind::PostfixBracket(_) => PostfixBracket => Accepted,
    HirExprKind::Error(_) => Error => RejectOnly,
    HirExprKind::ForSynthetic(_) => ForSynthetic => Accepted,
});

expression_family_inventory!(ExpressionResolutionFamily for CheckedExpressionResolution, {
    CheckedExpressionResolution::Structural => Structural => Accepted,
    CheckedExpressionResolution::Scope(_) => Scope => Accepted,
    CheckedExpressionResolution::Literal(_) => Literal => Accepted,
    CheckedExpressionResolution::Value(_) => Value => Accepted,
    CheckedExpressionResolution::Select(_) => Select => Accepted,
    CheckedExpressionResolution::Nominal(_) => Nominal => Accepted,
    CheckedExpressionResolution::Variant(_) => Variant => Accepted,
    CheckedExpressionResolution::CompileTimeEnum(_) => CompileTimeEnum => Accepted,
    CheckedExpressionResolution::StageLook(_) => StageLook => Accepted,
    CheckedExpressionResolution::Effect(_) => Effect => ProvenUnreachable,
    CheckedExpressionResolution::Call => Call => Accepted,
    CheckedExpressionResolution::Await(_) => Await => Accepted,
    CheckedExpressionResolution::Choice(_) => Choice => Accepted,
    CheckedExpressionResolution::Try(_) => Try => Accepted,
    CheckedExpressionResolution::ImplicitCallable(_) => ImplicitCallable => Accepted,
    CheckedExpressionResolution::Closure(_) => Closure => Accepted,
    CheckedExpressionResolution::ImplicitParameter(_) => ImplicitParameter => Accepted,
    CheckedExpressionResolution::Pipe(_) => Pipe => Accepted,
    CheckedExpressionResolution::PipeLeft(_) => PipeLeft => Accepted,
    CheckedExpressionResolution::ViewCall(_) => ViewCall => Accepted,
    CheckedExpressionResolution::ViewFxApplication(_) => ViewFxApplication => Accepted,
    CheckedExpressionResolution::StyleValue(_) => StyleValue => ProvenUnreachable,
    CheckedExpressionResolution::CompileTimeCallee(_) => CompileTimeCallee => Accepted,
    CheckedExpressionResolution::TypeValue(_) => TypeValue => Accepted,
    CheckedExpressionResolution::CompileTimeScalar(_) => CompileTimeScalar => Accepted,
    CheckedExpressionResolution::DialogueLineReference(_) => DialogueLineReference => Accepted,
    CheckedExpressionResolution::DialogueLineCoordinate(_) => DialogueLineCoordinate => Accepted,
    CheckedExpressionResolution::DialogueTextKeyCoordinate(_) => DialogueTextKeyCoordinate => Accepted,
    CheckedExpressionResolution::CharacterDialogueFactory(_) => CharacterDialogueFactory => Accepted,
    CheckedExpressionResolution::CharacterDialogueReconfigure(_) => CharacterDialogueReconfigure => Accepted,
    CheckedExpressionResolution::DialogueApplication { .. } => DialogueApplication => Accepted,
    CheckedExpressionResolution::ContentApplication(_) => ContentApplication => Accepted,
    CheckedExpressionResolution::PostfixBracket(_) => PostfixBracket => Accepted,
});

expression_family_inventory!(ValueResolutionFamily for CheckedValueResolution, {
    CheckedValueResolution::Local(_) => Local => Accepted,
    CheckedValueResolution::LineContext => LineContext => Accepted,
    CheckedValueResolution::CharacterField { .. } => CharacterField => Accepted,
    CheckedValueResolution::ProjectCallable(_) => ProjectCallable => Accepted,
    CheckedValueResolution::ProjectItem(_) => ProjectItem => Accepted,
    CheckedValueResolution::Entry(_) => Entry => Accepted,
    CheckedValueResolution::Registered(_) => Registered => Accepted,
    CheckedValueResolution::Constant(_) => Constant => Accepted,
});

expression_family_inventory!(SelectResolutionFamily for CheckedSelectResolution, {
    CheckedSelectResolution::Method(_) => Method => Accepted,
    CheckedSelectResolution::DialogueView { .. } => DialogueView => Accepted,
    CheckedSelectResolution::AgentField { .. } => AgentField => Accepted,
    CheckedSelectResolution::ProgressField { .. } => ProgressField => Accepted,
    CheckedSelectResolution::Field(_) => Field => Accepted,
});

#[derive(Default)]
struct MatchExpressionCorpusObservation {
    shapes: BTreeSet<ExprShapeFamily>,
    resolutions: BTreeSet<ExpressionResolutionFamily>,
    values: BTreeSet<ValueResolutionFamily>,
    selects: BTreeSet<SelectResolutionFamily>,
    value_facts: Vec<(CheckedValueResolution, TypeKind)>,
    select_facts: Vec<(CheckedSelectResolution, TypeKind)>,
    scope_facts: Vec<crate::final_analysis::CheckedScopeIdentity>,
    variant_facts: Vec<crate::final_analysis::CheckedVariantResolution>,
    match_type: Option<TypeKind>,
    semantic_digest: Option<[u8; 32]>,
}

impl MatchExpressionCorpusObservation {
    fn record_value(&mut self, value: &CheckedValueResolution) {
        self.values.insert(ValueResolutionFamily::of(value));
        if let CheckedValueResolution::CharacterField { receiver, .. } = value {
            self.record_value(receiver);
        }
    }

    fn record_resolution(&mut self, resolution: &CheckedExpressionResolution) {
        self.resolutions
            .insert(ExpressionResolutionFamily::of(resolution));
        if let CheckedExpressionResolution::Value(value) = resolution {
            self.record_value(value);
        }
        if let CheckedExpressionResolution::Select(select) = resolution {
            self.selects.insert(SelectResolutionFamily::of(select));
        }
        if let CheckedExpressionResolution::Variant(variant) = resolution {
            self.variant_facts.push(variant.clone());
        }
        if let CheckedExpressionResolution::Scope(scope) = resolution {
            self.scope_facts.push(scope.clone());
        }
        if let CheckedExpressionResolution::CompileTimeScalar(scalar) = resolution {
            self.record_resolution(scalar.original());
        }
        if let CheckedExpressionResolution::ImplicitCallable(callable) = resolution
            && let CheckedImplicitCallableBody::Plain(inner) = callable.body()
        {
            self.record_resolution(inner);
        }
    }
}

/// Traverse only expressions whose accepted path descends from one Match root.
/// Checked wrappers may retain a nested Value/Select resolution at that owner.
fn accepted_match_expression_corpus_observation(source: &str) -> MatchExpressionCorpusObservation {
    let world = super::fixture(source, None);
    accepted_match_expression_corpus_observation_for_fixture(&world)
}

fn accepted_match_expression_corpus_observation_for_fixture(
    world: &super::Fixture,
) -> MatchExpressionCorpusObservation {
    let report = super::analyze(world).expect("expression corpus source should check");
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
        panic!("each expression corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let root_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut observation = MatchExpressionCorpusObservation {
        shapes: BTreeSet::new(),
        resolutions: BTreeSet::new(),
        values: BTreeSet::new(),
        selects: BTreeSet::new(),
        value_facts: Vec::new(),
        select_facts: Vec::new(),
        scope_facts: Vec::new(),
        variant_facts: Vec::new(),
        match_type: report
            .expression(*match_owner)
            .and_then(|checked| checked.value_type().cloned()),
        semantic_digest: Some(*product.semantic_digest().as_bytes()),
    };
    for (owner, hir) in module.expressions() {
        let Some(checked) = report.expression(owner) else {
            continue;
        };
        let path = coordinates
            .expression(owner)
            .expect("checked expression owner path");
        if path.root() != root_path.root() || !path.steps().starts_with(root_path.steps()) {
            continue;
        }
        observation.shapes.insert(ExprShapeFamily::of(hir.kind()));
        observation.record_resolution(checked.resolution());
        if let CheckedExpressionResolution::Value(value) = checked.resolution() {
            observation.value_facts.push((
                value.clone(),
                checked
                    .value_type()
                    .expect("checked Value expression has a type")
                    .clone(),
            ));
        }
        if let CheckedExpressionResolution::Select(select) = checked.resolution() {
            observation.select_facts.push((
                select.clone(),
                checked
                    .value_type()
                    .expect("checked Select expression has a type")
                    .clone(),
            ));
        }
    }
    assert!(observation.shapes.contains(&ExprShapeFamily::Match));
    observation
}

fn assert_expression_corpus_inventory<T: Copy + Ord + std::fmt::Debug>(
    axis: &str,
    observed: &BTreeSet<T>,
    inventory: &[(T, ExpressionCorpusDisposition)],
) {
    for (family, disposition) in inventory {
        assert_eq!(
            observed.contains(family),
            *disposition == ExpressionCorpusDisposition::Accepted,
            "{axis} {family:?} corpus disposition {disposition:?}",
        );
    }
}

struct ExpressionCorpusRow {
    name: &'static str,
    exact_path_families: bool,
    source: String,
    fixture: ExpressionCorpusFixture,
    shapes: &'static [ExprShapeFamily],
    resolutions: &'static [ExpressionResolutionFamily],
    values: &'static [ValueResolutionFamily],
    selects: &'static [SelectResolutionFamily],
}

#[derive(Clone, Copy)]
enum ExpressionCorpusFixture {
    Standard,
    ExternalCharacter,
    CharacterNominal,
    RegisteredI32Pair,
    RegisteredObservation,
}

impl ExpressionCorpusFixture {
    fn build(self, source: &str) -> super::Fixture {
        match self {
            Self::Standard => super::fixture(source, None),
            Self::ExternalCharacter => super::external_character_fixture(source),
            Self::CharacterNominal => super::character_nominal_fixture(source),
            Self::RegisteredI32Pair => super::fixture_with_base_environment(
                source,
                None,
                TypeCheckEnv::standard()
                    .with_symbol("registered_left", TypeKind::I32)
                    .with_symbol("registered_right", TypeKind::I32),
            ),
            Self::RegisteredObservation => super::fixture_with_base_environment(
                source,
                None,
                TypeCheckEnv::standard().with_symbol("observation", TypeKind::Observation),
            ),
        }
    }
}

fn view_method_corpus_row() -> ExpressionCorpusRow {
    ExpressionCorpusRow {
        name: "selected view method",
        exact_path_families: false,
        source: method_match_source("", "Button().on_click { dialogue.primary_action }"),
        fixture: ExpressionCorpusFixture::Standard,
        shapes: &[
            ExprShapeFamily::Call,
            ExprShapeFamily::Select,
            ExprShapeFamily::Closure,
        ],
        resolutions: &[
            ExpressionResolutionFamily::Select,
            ExpressionResolutionFamily::Call,
            ExpressionResolutionFamily::Closure,
            ExpressionResolutionFamily::ViewCall,
            ExpressionResolutionFamily::CompileTimeCallee,
        ],
        values: &[],
        selects: &[
            SelectResolutionFamily::Method,
            SelectResolutionFamily::DialogueView,
        ],
    }
}

fn ordinary_expression_corpus_cases() -> Vec<(ExpressionCorpusRow, String)> {
    use ExprShapeFamily::{
        Block, Borrow, ComputationBlock, Dereference, ForSynthetic, If, IfLet, Literal, Loop,
        Match, NumericBracketSequence, Path, Record, RecordLiteral, Unary, Unit,
    };
    const STRUCTURAL: &[ExpressionResolutionFamily] = &[
        ExpressionResolutionFamily::Structural,
        ExpressionResolutionFamily::Literal,
        ExpressionResolutionFamily::Value,
    ];
    const CARRIER: &[ExpressionResolutionFamily] = &[
        ExpressionResolutionFamily::Structural,
        ExpressionResolutionFamily::Literal,
        ExpressionResolutionFamily::Value,
        ExpressionResolutionFamily::Variant,
    ];
    const NOMINAL: &[ExpressionResolutionFamily] = &[
        ExpressionResolutionFamily::Structural,
        ExpressionResolutionFamily::Literal,
        ExpressionResolutionFamily::Value,
        ExpressionResolutionFamily::Nominal,
    ];
    let nominal = |expression: &str| {
        format!(
            "struct Pair {{ first: i64, second: bool }}\n\
         fn root(flag: bool) -> Pair {{\n\
             match flag {{\n\
                 true => {expression}\n\
                 false => Pair {{ first = 0i64, second = false }}\n\
             }}\n\
         }}\n"
        )
    };
    let carrier = |value: &str| {
        format!(
            "fn root(flag: bool) -> Option<i64> {{\n\
             match flag {{\n\
                 true => option {{ {value} }}\n\
                 false => None\n\
             }}\n\
         }}\n"
        )
    };
    let iteration = |values: &str| {
        format!(
            "flow root(flag: bool) {{\n\
             let selected = match flag {{\n\
                 true => {{ for value in [{values}] {{}}; 1i64 }}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
        )
    };
    let cases: Vec<(
        &str,
        String,
        String,
        &[ExprShapeFamily],
        &[ExpressionResolutionFamily],
    )> = vec![
        (
            "unary",
            bool_match_i64_source("-1i64"),
            bool_match_i64_source("-2i64"),
            &[Literal, Path, Unary, Match],
            STRUCTURAL,
        ),
        (
            "If expression",
            bool_match_i64_source("if flag { 1i64 } else { 2i64 }"),
            bool_match_i64_source("if flag { 3i64 } else { 2i64 }"),
            &[Literal, Path, Block, If, Match],
            STRUCTURAL,
        ),
        (
            "IfLet expression",
            bool_match_i64_source("if let true = flag { 1i64 } else { 2i64 }"),
            bool_match_i64_source("if let true = flag { 3i64 } else { 2i64 }"),
            &[Literal, Path, Block, IfLet, Match],
            STRUCTURAL,
        ),
        (
            "Loop expression",
            bool_match_i64_source("loop { break 1i64 }"),
            bool_match_i64_source("loop { break 2i64 }"),
            &[Unit, Literal, Path, Loop, Match],
            STRUCTURAL,
        ),
        (
            "carrier block",
            carrier("1i64"),
            carrier("2i64"),
            &[Literal, Path, ComputationBlock, Match],
            CARRIER,
        ),
        (
            "named record",
            nominal("Pair { first = 1i64, second = true }"),
            nominal("Pair { first = 2i64, second = true }"),
            &[Literal, Path, Record, Match],
            NOMINAL,
        ),
        (
            "contextual record",
            nominal("({ first = 1i64, second = true })"),
            nominal("({ first = 2i64, second = true })"),
            &[Literal, Path, Record, RecordLiteral, Match],
            NOMINAL,
        ),
        (
            "borrow and dereference",
            bool_match_i64_source("{ let borrowed = &1i64; *borrowed }"),
            bool_match_i64_source("{ let borrowed = &2i64; *borrowed }"),
            &[Literal, Path, Borrow, Dereference, Block, Match],
            STRUCTURAL,
        ),
        (
            "For synthetic chain",
            iteration("1i64, 2i64"),
            iteration("3i64, 2i64"),
            &[
                Literal,
                Path,
                NumericBracketSequence,
                Block,
                Match,
                ForSynthetic,
            ],
            STRUCTURAL,
        ),
    ];
    cases
        .into_iter()
        .map(|(name, source, changed, shapes, resolutions)| {
            (
                ExpressionCorpusRow {
                    name,
                    exact_path_families: true,
                    source,
                    fixture: ExpressionCorpusFixture::Standard,
                    shapes,
                    resolutions,
                    values: &[],
                    selects: &[],
                },
                changed,
            )
        })
        .collect()
}

#[test]
fn checked_match_ordinary_expression_corpus_retains_families_and_meaning() {
    for (row, changed) in ordinary_expression_corpus_cases() {
        let first = accepted_match_expression_corpus_observation(&row.source);
        let second = accepted_match_expression_corpus_observation(&changed);
        assert_eq!(
            first.shapes,
            row.shapes.iter().copied().collect(),
            "{} exact shapes",
            row.name
        );
        assert_eq!(
            first.resolutions,
            row.resolutions.iter().copied().collect(),
            "{} exact resolutions",
            row.name
        );
        assert_eq!(first.shapes, second.shapes, "{} keeps shape", row.name);
        assert_eq!(
            first.resolutions, second.resolutions,
            "{} keeps resolution",
            row.name
        );
        assert_eq!(
            first
                .match_type
                .as_ref()
                .map(|ty| ty.semantic_identity_digest().expect("closed Match type")),
            second
                .match_type
                .as_ref()
                .map(|ty| ty.semantic_identity_digest().expect("closed Match type")),
            "{} keeps value type",
            row.name
        );
        assert_ne!(
            first.semantic_digest, second.semantic_digest,
            "{} retains meaning",
            row.name
        );
        let revised = format!(
            "fn unrelated() -> i64 {{ 99i64 }}\n{}",
            row.source.replace("true =>", "true  =>  ")
        );
        let original_match = outermost(&match_observations(&row.source));
        let revised_match = outermost(&match_observations(&revised));
        assert_ne!(
            original_match.owner, revised_match.owner,
            "{} changes raw HIR ID",
            row.name
        );
        assert_ne!(
            original_match.source_start, revised_match.source_start,
            "{} changes span",
            row.name
        );
        assert_eq!(
            original_match.digest, revised_match.digest,
            "{} preserves meaning",
            row.name
        );
    }
}

fn expression_corpus_rows() -> Vec<ExpressionCorpusRow> {
    let mut rows = vec![
        ExpressionCorpusRow {
            name: "binary scalar",
            exact_path_families: false,
            source: bool_match_i64_source("1i64 + 2i64"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Match,
                ExprShapeFamily::Binary,
                ExprShapeFamily::Literal,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "compact numeric sequence",
            exact_path_families: false,
            source: numeric_sequence_match_source("[1i64, 2i64]"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::NumericBracketSequence],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "named scope expression",
            exact_path_families: false,
            source: bool_match_i64_source("scope local { 1i64 }"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::NamedBlock],
            resolutions: &[ExpressionResolutionFamily::Scope],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "local and project callable values",
            exact_path_families: false,
            source: callable_value_match_source("saved"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Block, ExprShapeFamily::Path],
            resolutions: &[ExpressionResolutionFamily::Value],
            values: &[
                ValueResolutionFamily::Local,
                ValueResolutionFamily::ProjectCallable,
            ],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "record field",
            exact_path_families: false,
            source: field_match_source("left"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Select],
            resolutions: &[ExpressionResolutionFamily::Select],
            values: &[],
            selects: &[SelectResolutionFamily::Field],
        },
        view_method_corpus_row(),
        ExpressionCorpusRow {
            name: "checked Try carrier",
            exact_path_families: false,
            source: try_match_source("first"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Try],
            resolutions: &[ExpressionResolutionFamily::Try],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "pipeline placeholders",
            exact_path_families: false,
            source: pipe_match_source("identity"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Pipe,
                ExprShapeFamily::Placeholder,
                ExprShapeFamily::Tuple,
                ExprShapeFamily::Call,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Pipe,
                ExpressionResolutionFamily::PipeLeft,
                ExpressionResolutionFamily::Call,
            ],
            values: &[ValueResolutionFamily::ProjectCallable],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "Choice plan with thread value",
            exact_path_families: false,
            source: choice_match_source("", "with { window = thread {} }"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Choice,
                ExprShapeFamily::Thread,
                ExprShapeFamily::Unit,
            ],
            resolutions: &[ExpressionResolutionFamily::Choice],
            values: &[],
            selects: &[],
        },
    ];
    rows.extend(checked_owner_expression_corpus_rows());
    rows.extend(
        ordinary_expression_corpus_cases()
            .into_iter()
            .map(|(row, _)| row),
    );
    rows.extend(contextual_expressions::contextual_expression_corpus_rows());
    rows
}

fn checked_owner_expression_corpus_rows() -> Vec<ExpressionCorpusRow> {
    vec![
        ExpressionCorpusRow {
            name: "checked Entry references",
            exact_path_families: false,
            source: entry_reference_match_source("@entry.cli.primary", "@entry.cli.alternate"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::EntityReference],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::Entry],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "checked external and project Character items",
            exact_path_families: false,
            source: project_item_match_source("@character.akane", "@character.alternate"),
            fixture: ExpressionCorpusFixture::ExternalCharacter,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::EntityReference],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::ProjectItem],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "registered environment value",
            exact_path_families: false,
            source: registered_value_match_source("registered_left"),
            fixture: ExpressionCorpusFixture::RegisteredI32Pair,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::Path],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Value,
            ],
            values: &[ValueResolutionFamily::Registered],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "Await Pending Progress field",
            exact_path_families: false,
            source: progress_field_match_source("ratio"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::Select],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Select,
            ],
            values: &[],
            selects: &[SelectResolutionFamily::ProgressField],
        },
        ExpressionCorpusRow {
            name: "Dialogue line context value",
            exact_path_families: false,
            source: dialogue_line_context_match_source(),
            fixture: ExpressionCorpusFixture::CharacterNominal,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::Call],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Call,
                ExpressionResolutionFamily::Value,
            ],
            values: &[ValueResolutionFamily::LineContext],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "Dialogue Character Stage field value",
            exact_path_families: false,
            source: dialogue_character_field_match_source("akane"),
            fixture: ExpressionCorpusFixture::CharacterNominal,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::Call],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Call,
                ExpressionResolutionFamily::Variant,
                ExpressionResolutionFamily::Value,
            ],
            values: &[
                ValueResolutionFamily::CharacterField,
                ValueResolutionFamily::ProjectItem,
            ],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "generation Match-root dialogue calls",
            exact_path_families: true,
            source: super::generation_invariance::dialogue_call_match_source("", "project_action"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Literal,
                ExprShapeFamily::Path,
                ExprShapeFamily::Call,
                ExprShapeFamily::Block,
                ExprShapeFamily::Match,
                ExprShapeFamily::AttachedContentApplication,
                ExprShapeFamily::PostfixBracket,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Call,
                ExpressionResolutionFamily::DialogueApplication,
                ExpressionResolutionFamily::PostfixBracket,
            ],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "generation Match-root attached content modifier",
            exact_path_families: true,
            source: super::generation_invariance::content_call_match_source("", "strong"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Literal,
                ExprShapeFamily::Path,
                ExprShapeFamily::Block,
                ExprShapeFamily::Match,
                ExprShapeFamily::AttachedContentApplication,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::DialogueApplication,
                ExpressionResolutionFamily::ContentApplication,
            ],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "generation Match-root project content call",
            exact_path_families: true,
            source: super::generation_invariance::project_content_call_match_source(
                "",
                "passthrough",
            ),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Literal,
                ExprShapeFamily::Path,
                ExprShapeFamily::Block,
                ExprShapeFamily::Match,
                ExprShapeFamily::AttachedContentApplication,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::DialogueApplication,
                ExpressionResolutionFamily::ContentApplication,
            ],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "generation Match-root project Fx content call",
            exact_path_families: true,
            source: super::generation_invariance::project_fx_match_source("", "#ff6b8a"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Literal,
                ExprShapeFamily::Path,
                ExprShapeFamily::Call,
                ExprShapeFamily::Block,
                ExprShapeFamily::Match,
                ExprShapeFamily::AttachedContentApplication,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Call,
                ExpressionResolutionFamily::CompileTimeScalar,
                ExpressionResolutionFamily::DialogueApplication,
                ExpressionResolutionFamily::ContentApplication,
            ],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "generation Match-root View Fx application",
            exact_path_families: true,
            source: super::generation_invariance::view_fx_match_source("", "wave(speed = speed)"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Literal,
                ExprShapeFamily::Path,
                ExprShapeFamily::Call,
                ExprShapeFamily::Select,
                ExprShapeFamily::Match,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Select,
                ExpressionResolutionFamily::Call,
                ExpressionResolutionFamily::ViewCall,
                ExpressionResolutionFamily::ViewFxApplication,
                ExpressionResolutionFamily::CompileTimeCallee,
            ],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "ordinary Match-root range value",
            exact_path_families: true,
            source: range_match_source(false),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Path,
                ExprShapeFamily::Literal,
                ExprShapeFamily::Range,
                ExprShapeFamily::Match,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Structural,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "ordinary Match-root bracket sequence",
            exact_path_families: true,
            source: bracket_sequence_match_source("true, false", "false, true"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Path,
                ExprShapeFamily::Literal,
                ExprShapeFamily::BracketSequence,
                ExprShapeFamily::Match,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Structural,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "ordinary Match-root array repeat",
            exact_path_families: true,
            source: array_repeat_match_source("0"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Path,
                ExprShapeFamily::Literal,
                ExprShapeFamily::ArrayRepeat,
                ExprShapeFamily::Match,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Structural,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "ordinary Match-root Vec index",
            exact_path_families: true,
            source: index_match_source(0),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Path,
                ExprShapeFamily::Literal,
                ExprShapeFamily::Index,
                ExprShapeFamily::Match,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Value,
                ExpressionResolutionFamily::Literal,
                ExpressionResolutionFamily::Structural,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
    ]
}

#[test]
fn checked_match_expression_corpus_tracks_accepted_root_families() {
    let mut observed = MatchExpressionCorpusObservation::default();
    for row in expression_corpus_rows() {
        let world = row.fixture.build(&row.source);
        let found = accepted_match_expression_corpus_observation_for_fixture(&world);
        if row.exact_path_families {
            assert_eq!(
                found.shapes,
                row.shapes.iter().copied().collect(),
                "{} has the observed complete accepted-path HIR shape set",
                row.name,
            );
            assert_eq!(
                found.resolutions,
                row.resolutions.iter().copied().collect(),
                "{} has the observed complete accepted-path resolution set",
                row.name,
            );
        }
        for required in row.shapes {
            assert!(
                found.shapes.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.resolutions {
            assert!(
                found.resolutions.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.values {
            assert!(
                found.values.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.selects {
            assert!(
                found.selects.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        observed.shapes.extend(found.shapes);
        observed.resolutions.extend(found.resolutions);
        observed.values.extend(found.values);
        observed.selects.extend(found.selects);
    }
    assert_expression_corpus_inventory(
        "HIR expression",
        &observed.shapes,
        ExprShapeFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked expression",
        &observed.resolutions,
        ExpressionResolutionFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked Value",
        &observed.values,
        ValueResolutionFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked Select",
        &observed.selects,
        SelectResolutionFamily::INVENTORY,
    );
}

#[test]
fn checked_match_transcript_commits_entry_value_owner() {
    let entry_world = ExpressionCorpusFixture::Standard.build(&entry_reference_match_source(
        "@entry.cli.primary",
        "@entry.cli.alternate",
    ));
    let entries = accepted_match_expression_corpus_observation_for_fixture(&entry_world);
    let checked_entries = entries
        .value_facts
        .iter()
        .filter_map(|(value, ty)| match value {
            CheckedValueResolution::Entry(entry) => Some((entry, ty)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let entry_ids = checked_entries
        .iter()
        .map(|(entry, ty)| {
            assert_eq!(*ty, &TypeKind::entity_ref(EntityKind::Entry));
            assert_eq!(
                entry.value_type(),
                TypeKind::entity_ref(EntityKind::Entry)
                    .semantic_identity_digest()
                    .expect("Entry reference type has a stable identity"),
            );
            entry.diagnostic_public_id().as_str().to_owned()
        })
        .collect::<BTreeSet<_>>();
    let entry_bindings = checked_entries
        .iter()
        .map(|(entry, _)| *entry.binding().as_bytes())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entry_ids,
        ["entry.cli.primary", "entry.cli.alternate"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "both arm values resolve through the accepted Entry catalog",
    );
    assert_eq!(
        entry_bindings.len(),
        2,
        "each Entry has its own checked binding"
    );

    let same_entry_world = ExpressionCorpusFixture::Standard.build(&entry_reference_match_source(
        "@entry.cli.primary",
        "@entry.cli.primary",
    ));
    let same_entry = accepted_match_expression_corpus_observation_for_fixture(&same_entry_world);
    assert_ne!(
        entries.semantic_digest, same_entry.semantic_digest,
        "changing one Match arm's checked Entry owner changes its transcript",
    );
}

#[test]
fn checked_match_transcript_commits_project_item_value_owner() {
    let project_item_world = ExpressionCorpusFixture::ExternalCharacter.build(
        &project_item_match_source("@character.akane", "@character.alternate"),
    );
    let project_items =
        accepted_match_expression_corpus_observation_for_fixture(&project_item_world);
    let checked_items = project_items
        .value_facts
        .iter()
        .filter_map(|(value, ty)| match value {
            CheckedValueResolution::ProjectItem(item) => Some((item, ty)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let character_type = TypeKind::entity_ref(EntityKind::Character);
    let project_item_ids = checked_items
        .iter()
        .map(|(item, ty)| {
            assert_eq!(*ty, &character_type);
            assert_eq!(
                item.value_type(),
                character_type
                    .semantic_identity_digest()
                    .expect("Character reference type has a stable identity")
            );
            item.semantic_id()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        project_item_ids.len(),
        2,
        "the arms retain distinct checked Character owners"
    );
    let external = checked_items
        .iter()
        .find(|(item, _)| item.public_id().as_str() == "character.akane")
        .expect("external registered Character item");
    assert!(external.0.external_declaration().is_some());
    let retained = checked_items
        .iter()
        .find(|(item, _)| item.public_id().as_str() == "character.alternate")
        .expect("retained project Character item");
    assert!(retained.0.retained_owner().is_some());

    let same_project_item_world = ExpressionCorpusFixture::ExternalCharacter.build(
        &project_item_match_source("@character.akane", "@character.akane"),
    );
    let same_project_item =
        accepted_match_expression_corpus_observation_for_fixture(&same_project_item_world);
    assert_ne!(
        project_items.semantic_digest, same_project_item.semantic_digest,
        "changing one Match arm's checked ProjectItem owner changes its transcript",
    );
}

fn registered_value_fact(
    observation: &MatchExpressionCorpusObservation,
) -> (&RegisteredSemanticValueId, &TypeKind) {
    observation
        .value_facts
        .iter()
        .find_map(|(value, ty)| match value {
            CheckedValueResolution::Registered(value) => Some((value, ty)),
            _ => None,
        })
        .expect("Match descendant has a checked Registered value")
}

fn line_context_facts(observation: &MatchExpressionCorpusObservation) -> Vec<&TypeKind> {
    observation
        .value_facts
        .iter()
        .filter_map(|(value, ty)| {
            matches!(value, CheckedValueResolution::LineContext).then_some(ty)
        })
        .collect()
}

fn character_field_facts(
    observation: &MatchExpressionCorpusObservation,
) -> Vec<(
    &CharacterId,
    &crate::types::CharacterField,
    &CheckedValueResolution,
    &TypeKind,
)> {
    observation
        .value_facts
        .iter()
        .filter_map(|(value, ty)| match value {
            CheckedValueResolution::CharacterField {
                receiver,
                character,
                field,
            } => Some((character, field, receiver.as_ref(), ty)),
            _ => None,
        })
        .collect()
}

fn variant_facts(
    observation: &MatchExpressionCorpusObservation,
) -> &[crate::final_analysis::CheckedVariantResolution] {
    &observation.variant_facts
}

fn assert_dialogue_match_source_revision_invariant(
    source: &str,
    fixture: ExpressionCorpusFixture,
    original_digest: [u8; 32],
) {
    let revised_source = format!("fn unrelated() -> i64 {{ 99i64 }}\n{source}");
    let revised_world = fixture.build(&revised_source);
    let revised = accepted_match_expression_corpus_observation_for_fixture(&revised_world);
    let original_world = fixture.build(source);
    let original = outermost(&match_observations_for_fixture(&original_world));
    let revised_match = outermost(&match_observations_for_fixture(&revised_world));
    assert_eq!(original_digest, original.digest);
    assert_ne!(original.owner, revised_match.owner);
    assert_ne!(original.source_start, revised_match.source_start);
    assert_ne!(original.source_end, revised_match.source_end);
    assert_eq!(
        original.digest,
        revised.semantic_digest.expect("checked Match digest")
    );
}

#[test]
fn checked_match_transcript_reaches_dialogue_line_context_values() {
    let source = dialogue_line_context_match_source();
    let world = ExpressionCorpusFixture::CharacterNominal.build(&source);
    let observation = accepted_match_expression_corpus_observation_for_fixture(&world);
    let facts = line_context_facts(&observation);
    assert_eq!(facts.len(), 2, "each Match arm has a checked line receiver");
    assert!(facts.iter().all(|ty| **ty == TypeKind::LineContext));
    assert_eq!(observation.match_type, Some(TypeKind::VoiceHandle));

    assert_dialogue_match_source_revision_invariant(
        &source,
        ExpressionCorpusFixture::CharacterNominal,
        observation.semantic_digest.expect("checked Match digest"),
    );
}

#[test]
fn checked_match_transcript_reaches_dialogue_character_field_values() {
    let akane_source = dialogue_character_field_match_source("akane");
    let akane_world = ExpressionCorpusFixture::CharacterNominal.build(&akane_source);
    let akane = accepted_match_expression_corpus_observation_for_fixture(&akane_world);
    let akane_id = CharacterId::try_new("character.akane").expect("Akane Character ID");
    let akane_facts = character_field_facts(&akane);
    assert_eq!(
        akane_facts.len(),
        2,
        "each Match arm has a checked Stage field"
    );
    for (character, field, receiver, ty) in &akane_facts {
        assert_eq!(*character, &akane_id);
        assert_eq!(*field, &crate::types::CharacterField::Stage);
        assert_eq!(
            *ty,
            &TypeKind::StageApi(akane_id.clone()),
            "the exact Character owner is part of the StageApi type",
        );
        let CheckedValueResolution::ProjectItem(item) = receiver else {
            panic!("Character Stage receiver retains a checked project Character item");
        };
        assert_eq!(item.character(), Some(akane_id.clone()));
        assert!(item.retained_owner().is_some());
    }
    assert_eq!(
        akane.match_type,
        Some(TypeKind::StageActorHandle(StageActorHandleType::Exact(
            akane_id.clone()
        )))
    );
    assert_scope_line_variant_facts(&akane);

    let alternate_source = dialogue_character_field_match_source("alternate");
    let alternate_world = ExpressionCorpusFixture::CharacterNominal.build(&alternate_source);
    let alternate = accepted_match_expression_corpus_observation_for_fixture(&alternate_world);
    let alternate_id = CharacterId::try_new("character.alternate").expect("Alternate Character ID");
    let alternate_facts = character_field_facts(&alternate);
    assert_eq!(
        alternate_facts.len(),
        2,
        "each alternate-owner Match arm has a checked Stage field",
    );
    for (character, field, receiver, ty) in &alternate_facts {
        assert_eq!(*character, &alternate_id);
        assert_eq!(*field, &crate::types::CharacterField::Stage);
        assert_eq!(*ty, &TypeKind::StageApi(alternate_id.clone()));
        let CheckedValueResolution::ProjectItem(item) = receiver else {
            panic!("alternate Character Stage receiver remains checked");
        };
        assert_eq!(item.character(), Some(alternate_id.clone()));
        assert!(item.retained_owner().is_some());
    }
    assert_eq!(
        alternate.match_type,
        Some(TypeKind::StageActorHandle(StageActorHandleType::Exact(
            alternate_id
        )))
    );
    assert_scope_line_variant_facts(&alternate);
    // Exact Character identity is carried by StageApi and StageActorHandle,
    // so this owner comparison also changes the checked Match result type.
    assert_ne!(akane.match_type, alternate.match_type);
    assert_ne!(akane.semantic_digest, alternate.semantic_digest);

    assert_dialogue_match_source_revision_invariant(
        &akane_source,
        ExpressionCorpusFixture::CharacterNominal,
        akane.semantic_digest.expect("checked Match digest"),
    );
}

fn assert_scope_line_variant_facts(observation: &MatchExpressionCorpusObservation) {
    let facts = variant_facts(observation);
    assert_eq!(facts.len(), 2, "each Match arm checks its scope variant");
    for variant in facts {
        let CheckedVariantOwnerKind::BuiltinClosed { nominal, ty } = variant.owner().kind() else {
            panic!("scope=line retains its closed PresentationLifetime owner");
        };
        assert_eq!(nominal.as_str(), "PresentationLifetime");
        let lifetime_type = TypeKind::Named("PresentationLifetime".to_owned());
        assert_eq!(ty, &lifetime_type);
        assert_eq!(variant.owner().ty(), lifetime_type);
        assert_eq!(variant.ordinal(), 3);
        assert_eq!(variant.selected().diagnostic_name(), Some("line"));
    }
    assert_eq!(
        facts[0], facts[1],
        "both arms select the same owner and case"
    );
}

#[test]
fn checked_match_transcript_commits_registered_environment_binding_identity() {
    let left_world = ExpressionCorpusFixture::RegisteredI32Pair
        .build(&registered_value_match_source("registered_left"));
    let left = accepted_match_expression_corpus_observation_for_fixture(&left_world);
    let (left_id, left_type) = registered_value_fact(&left);
    assert_eq!(left_type, &TypeKind::I32);
    assert_eq!(
        left_id
            .environment_binding()
            .expect("Registered value retains its environment binding")
            .as_str(),
        "registered_left",
    );

    let right_world = ExpressionCorpusFixture::RegisteredI32Pair
        .build(&registered_value_match_source("registered_right"));
    let right = accepted_match_expression_corpus_observation_for_fixture(&right_world);
    let (right_id, right_type) = registered_value_fact(&right);
    assert_eq!(right_type, left_type);
    assert_eq!(
        right_id
            .environment_binding()
            .expect("Registered value retains its environment binding")
            .as_str(),
        "registered_right",
    );
    assert_ne!(left_id.as_bytes(), right_id.as_bytes());
    assert_ne!(left.semantic_digest, right.semantic_digest);

    let revised_source = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        registered_value_match_source("registered_left"),
    );
    let revised_world = ExpressionCorpusFixture::RegisteredI32Pair.build(&revised_source);
    let revised = accepted_match_expression_corpus_observation_for_fixture(&revised_world);
    let (revised_id, revised_type) = registered_value_fact(&revised);
    assert_eq!(revised_type, left_type);
    assert_eq!(revised_id.as_bytes(), left_id.as_bytes());
    assert_eq!(revised.semantic_digest, left.semantic_digest);
}

fn progress_field_facts(
    observation: &MatchExpressionCorpusObservation,
) -> Vec<(&crate::types::ProgressField, &TypeKind)> {
    observation
        .select_facts
        .iter()
        .filter_map(|(selection, ty)| match selection {
            CheckedSelectResolution::ProgressField { field } => Some((field, ty)),
            _ => None,
        })
        .collect()
}

#[test]
fn checked_match_transcript_reaches_pending_progress_field_facts() {
    let ratio_world = super::fixture(&progress_field_match_source("ratio"), None);
    let ratio = accepted_match_expression_corpus_observation_for_fixture(&ratio_world);
    let ratio_facts = progress_field_facts(&ratio);
    assert_eq!(
        ratio_facts.len(),
        2,
        "both Match arms select Progress::ratio"
    );
    for (field, ty) in ratio_facts {
        assert_eq!(field, &crate::types::ProgressField::Ratio);
        assert_eq!(ty, &TypeKind::F32);
    }

    let label_world = super::fixture(&progress_field_match_source("label"), None);
    let label = accepted_match_expression_corpus_observation_for_fixture(&label_world);
    let label_facts = progress_field_facts(&label);
    assert_eq!(
        label_facts.len(),
        2,
        "both Match arms select Progress::label"
    );
    for (field, ty) in label_facts {
        assert_eq!(field, &crate::types::ProgressField::Label);
        assert_eq!(ty, &TypeKind::Option(Box::new(TypeKind::String)));
    }
    assert_ne!(TypeKind::F32, TypeKind::Option(Box::new(TypeKind::String)));
    assert_ne!(ratio.semantic_digest, label.semantic_digest);
}

#[test]
fn checked_match_named_block_commits_checked_namespace_identity() {
    let local = accepted_match_expression_corpus_observation(&bool_match_i64_source(
        "scope local { 1i64 }",
    ));
    let scene = accepted_match_expression_corpus_observation(&bool_match_i64_source(
        "scope scene { 1i64 }",
    ));
    let formatted = accepted_match_expression_corpus_observation(&bool_match_i64_source(
        "scope local {  1i64 }",
    ));

    for observation in [&local, &scene, &formatted] {
        assert!(observation.shapes.contains(&ExprShapeFamily::NamedBlock));
        assert!(
            observation
                .resolutions
                .contains(&ExpressionResolutionFamily::Scope)
        );
        assert_eq!(observation.scope_facts.len(), 1);
    }
    assert!(matches!(
        local.scope_facts.as_slice(),
        [crate::final_analysis::CheckedScopeIdentity::Named(name)] if name.as_str() == "local"
    ));
    assert!(matches!(
        scene.scope_facts.as_slice(),
        [crate::final_analysis::CheckedScopeIdentity::Named(name)] if name.as_str() == "scene"
    ));
    assert_eq!(local.scope_facts, formatted.scope_facts);
    assert_ne!(local.semantic_digest, scene.semantic_digest);
    assert_eq!(local.semantic_digest, formatted.semantic_digest);
}

#[test]
fn checked_match_transcript_commits_same_typed_sequence_repeat_and_index_values() {
    let sequence =
        |left: &str| source_match_digest(&bracket_sequence_match_source(left, "false, true"));
    assert_ne!(sequence("true, false"), sequence("false, false"));

    let repeat = |value: &str| source_match_digest(&array_repeat_match_source(value));
    assert_ne!(repeat("0"), repeat("1"));

    let index = |selected| source_match_digest(&index_match_source(selected));
    assert_ne!(index(0), index(1));
}

#[test]
fn effect_expression_fact_is_outside_its_function_body_match_path() {
    let source = r"
fn root(flag: bool) -> i64 effects { fs.read } {
    match flag {
        true => 1i64
        false => 0i64
    }
}
";
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("effect clause and Match body are accepted");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let matches = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = matches.as_slice() else {
        panic!("effect witness owns one Match expression");
    };
    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(*match_owner)
        .expect("accepted body Match path");
    let effect_paths = report
        .expressions()
        .filter_map(|(owner, checked)| {
            let CheckedExpressionResolution::Effect(effect) = checked.resolution() else {
                return None;
            };
            assert_eq!(effect.as_str(), "fs.read");
            Some(
                coordinates
                    .expression(owner)
                    .expect("declaration-contract Effect path"),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        !effect_paths.is_empty(),
        "the effect clause publishes checked facts"
    );
    assert!(
        effect_paths
            .iter()
            .all(|path| !path.is_at_or_below(&match_path))
    );
    assert!(effect_paths.iter().all(|path| matches!(
        path.steps().first(),
        Some(crate::semantic_coordinate::CheckedSemanticPathStep::DeclarationContract(_))
    )));
    assert!(matches!(
        match_path.steps().first(),
        Some(crate::semantic_coordinate::CheckedSemanticPathStep::DeclarationBody(_))
    ));

    let match_observation = accepted_match_expression_corpus_observation_for_fixture(&world);
    assert!(
        !match_observation
            .resolutions
            .contains(&ExpressionResolutionFamily::Effect)
    );
}

#[test]
fn checked_match_expression_corpus_excludes_unrelated_declaration_expressions() {
    let source = format!(
        "fn unrelated() -> i64 {{ let outside = 0i64..1i64; 0i64 }}\n{}",
        bool_match_i64_source("1i64 + 2i64"),
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated range should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.expressions().any(|(owner, expression)| {
        matches!(expression.kind(), HirExprKind::Range(_)) && report.expression(owner).is_some()
    }));
    let observation = accepted_match_expression_corpus_observation(&source);
    assert!(observation.shapes.contains(&ExprShapeFamily::Binary));
    assert!(
        !observation.shapes.contains(&ExprShapeFamily::Range),
        "an unrelated checked Range must not enter the Match corpus",
    );
}
