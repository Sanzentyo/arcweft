use arcweft_dialogue::InlineFailureSelection;
use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty, DialogueRichTextControl,
};
use arcweft_id::PublicId;
use arcweft_lang_hir::dialogue_application::{
    HirDialogueContentId, HirDialogueMarkName, HirDialoguePointActionArgumentId, HirLineBreakKind,
};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::source_index::HirSourceSite;
use arcweft_presentation::rich_text::{
    PresentationContentCallableDefinitionId, PresentationContentCallableParameterId,
};
use arcweft_rich_text_schema::RichTextCallableSchemaDigest;

use super::{
    CheckedAngle, CheckedDuration, CheckedLength, CheckedRichTextValue, Milli, RichTextDiagnostic,
};
use crate::semantic_coordinate::StableCheckedDialogueMarkCoordinate;
use crate::{
    callable::{CheckedAttachedContentAdmission, CheckedContentRole},
    semantic_coordinate::{
        CheckedExpressionCoordinateEvidence, CheckedSemanticPath,
        StableCheckedContentFragmentCoordinate, StableCheckedValueCoordinate,
    },
};

/// Stable identity of one materialized owner-schema default.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RichTextDefaultId(u16);

impl RichTextDefaultId {
    pub(crate) const fn from_schema_ordinal(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Closed property identity for a zero-width dialogue point action.
///
/// Body-bearing presentation operations do not use this property algebra;
/// they are checked as attached Content callable applications instead.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CheckedRichTextProperty {
    Control(DialogueControlProperty),
    Host(DialogueHostProperty),
}

/// Provenance of one checked point-action field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFieldOrigin {
    Authored {
        argument: HirDialoguePointActionArgumentId,
        key: Option<HirSourceSite>,
        value: HirSourceSite,
    },
    Defaulted {
        default_id: RichTextDefaultId,
    },
    /// Retained for the shared text-proxy default join. It is never used to
    /// represent a content-call parameter or to recover source spelling.
    TextProxyDefault {
        expression: ExprId,
    },
}

/// One property-identified typed point-action value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedField {
    property: CheckedRichTextProperty,
    value: CheckedRichTextValue,
    origin: CheckedFieldOrigin,
}

impl CheckedField {
    pub(crate) const fn new(
        property: CheckedRichTextProperty,
        value: CheckedRichTextValue,
        origin: CheckedFieldOrigin,
    ) -> Self {
        Self {
            property,
            value,
            origin,
        }
    }

    pub const fn property(&self) -> CheckedRichTextProperty {
        self.property
    }

    pub const fn value(&self) -> &CheckedRichTextValue {
        &self.value
    }

    pub const fn origin(&self) -> &CheckedFieldOrigin {
        &self.origin
    }
}

/// Deterministically schema-ordered values for one point-action owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedOwnerFields(Box<[CheckedField]>);

impl CheckedOwnerFields {
    pub(crate) fn new(fields: Vec<CheckedField>) -> Self {
        Self(fields.into_boxed_slice())
    }

    pub const fn fields(&self) -> &[CheckedField] {
        &self.0
    }
}

/// Resolved semantic owner of one checked zero-width point action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedRichTextOwner {
    Control(DialogueRichTextControl),
    Marker,
    Host(DialogueHostEventKind),
}

/// Exact typed point-control output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait { duration: CheckedDuration },
    Clear,
    Reset,
    RevealRate { milli_cps: Milli },
}

/// Checked voice-source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedVoiceSource {
    Auto,
    Identity(PublicId),
}

/// Exact checked host event after owner-schema validation.
///
/// Conditional spans are intentionally absent: branch structure is no longer
/// represented by a rich-text stack. Conditions belong to ordinary expression
/// and control-flow semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueHostEvent {
    Voice { source: CheckedVoiceSource },
    Face { expression: PublicId },
    Pose { pose: PublicId },
    Show { entity: PublicId },
    Hide { entity: PublicId },
    Move { x: CheckedLength, y: CheckedLength },
    Scale { x: Milli, y: Milli },
    Rotate { angle: CheckedAngle },
    Animation { animation: PublicId },
    Shake { amplitude: CheckedLength },
    TimedCue { at: CheckedDuration, call: ExprId },
    Call { call: ExprId },
    Signal { signal: PublicId },
}

/// Canonical pixel-milli depth accepted for one rich-text object proxy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedObjectDepth(i32);

impl CheckedObjectDepth {
    pub(crate) const fn new(milli: i32) -> Self {
        Self(milli)
    }

    pub const fn milli(self) -> i32 {
        self.0
    }
}

/// One typed parameter of a presentation Content callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentParameter {
    id: PresentationContentCallableParameterId,
    value: crate::final_analysis::CheckedCompileTimeValue,
}

impl CheckedContentParameter {
    pub(crate) const fn new(
        id: PresentationContentCallableParameterId,
        value: crate::final_analysis::CheckedCompileTimeValue,
    ) -> Self {
        Self { id, value }
    }

    pub const fn id(&self) -> PresentationContentCallableParameterId {
        self.id
    }

    pub const fn value(&self) -> &crate::final_analysis::CheckedCompileTimeValue {
        &self.value
    }
}

/// Checked renderer-neutral modifier application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentModifier {
    definition: PresentationContentCallableDefinitionId,
    schema: RichTextCallableSchemaDigest,
    parameters: Box<[CheckedContentParameter]>,
}

impl CheckedContentModifier {
    pub(crate) fn new(
        definition: PresentationContentCallableDefinitionId,
        schema: RichTextCallableSchemaDigest,
        parameters: Vec<CheckedContentParameter>,
    ) -> Self {
        Self {
            definition,
            schema,
            parameters: parameters.into_boxed_slice(),
        }
    }

    pub const fn definition(&self) -> PresentationContentCallableDefinitionId {
        self.definition
    }

    pub const fn schema(&self) -> RichTextCallableSchemaDigest {
        self.schema
    }

    pub const fn parameters(&self) -> &[CheckedContentParameter] {
        &self.parameters
    }
}

/// Typed ruby emission. The base is the attached checked body; only the
/// reading value is held in this carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentRuby {
    reading: Box<str>,
}

impl CheckedContentRuby {
    pub(crate) fn new(reading: impl Into<Box<str>>) -> Self {
        Self {
            reading: reading.into(),
        }
    }

    pub const fn reading(&self) -> &str {
        &self.reading
    }
}

/// Family-specific typed action consumed by compiler/runtime adapters.
///
/// This enum is deliberately limited to zero-width point actions. Presentation
/// modifiers, ruby, raw literals, and objects are `CheckedContentEmission`
/// values owned by their attached Content insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRichTextAction {
    Control {
        action: CheckedDialogueControl,
        fields: CheckedOwnerFields,
    },
    Host {
        owner: DialogueHostEventKind,
        action: CheckedDialogueHostEvent,
        fields: CheckedOwnerFields,
    },
    Marker(CheckedDialogueMark),
}

impl CheckedRichTextAction {
    pub const fn fields(&self) -> Option<&CheckedOwnerFields> {
        match self {
            Self::Control { fields, .. } | Self::Host { fields, .. } => Some(fields),
            Self::Marker(_) => None,
        }
    }
}

/// Final accepted identity of one dialogue-content marker.
///
/// Equality, ordering, and hashing deliberately exclude the display-only
/// diagnostic name. The accepted-rooted coordinate is the sole semantic
/// identity consumed by compiler projection and transcripts.
#[derive(Clone, Debug)]
pub struct CheckedDialogueMark {
    coordinate: StableCheckedDialogueMarkCoordinate,
    diagnostic_name: HirDialogueMarkName,
}

impl CheckedDialogueMark {
    pub(crate) const fn new(
        coordinate: StableCheckedDialogueMarkCoordinate,
        diagnostic_name: HirDialogueMarkName,
    ) -> Self {
        Self {
            coordinate,
            diagnostic_name,
        }
    }

    pub const fn coordinate(&self) -> &StableCheckedDialogueMarkCoordinate {
        &self.coordinate
    }

    pub const fn diagnostic_name(&self) -> &HirDialogueMarkName {
        &self.diagnostic_name
    }
}

impl PartialEq for CheckedDialogueMark {
    fn eq(&self, other: &Self) -> bool {
        self.coordinate == other.coordinate
    }
}

impl Eq for CheckedDialogueMark {}

impl PartialOrd for CheckedDialogueMark {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CheckedDialogueMark {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.coordinate.cmp(&other.coordinate)
    }
}

impl std::hash::Hash for CheckedDialogueMark {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.coordinate.hash(state);
    }
}

/// Raw literal text retained as a typed Content emission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRawLiteral {
    body: Box<str>,
}

impl CheckedRawLiteral {
    pub(crate) fn new(body: impl Into<Box<str>>) -> Self {
        Self { body: body.into() }
    }

    pub const fn body(&self) -> &str {
        &self.body
    }
}

/// Closed emission family for one checked attached-content application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedContentEmission {
    Modifier(CheckedContentModifier),
    Fx(crate::final_analysis::CheckedContentFxApplication),
    Ruby(CheckedContentRuby),
    Raw(CheckedRawLiteral),
    ObjectSpan(crate::checked_text_proxy::CheckedTextProxyApplication),
    ContentResult,
}

impl CheckedContentEmission {
    /// Returns whether this attached-content emission publishes a runtime
    /// value for its owning application expression.
    pub const fn produces_runtime_value(&self) -> bool {
        matches!(self, Self::ContentResult)
    }
}

/// Checked body supplied to one attached-content producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedAttachedContentArgument {
    Absent,
    Present {
        checked_content: Box<CheckedRichTextReport>,
    },
}

impl CheckedAttachedContentArgument {
    pub(crate) const fn absent() -> Self {
        Self::Absent
    }

    pub(crate) fn present(checked_content: CheckedRichTextReport) -> Self {
        Self::Present {
            checked_content: Box::new(checked_content),
        }
    }

    pub const fn admission(&self) -> Option<CheckedAttachedContentAdmission> {
        match self {
            Self::Absent => None,
            Self::Present { checked_content } => Some(checked_content.admission()),
        }
    }

    pub const fn role(&self) -> Option<CheckedContentRole> {
        match self.admission() {
            Some(admission) => admission.role(),
            None => None,
        }
    }

    pub fn checked_content(&self) -> Option<&CheckedRichTextReport> {
        match self {
            Self::Absent => None,
            Self::Present { checked_content } => Some(checked_content),
        }
    }
}

/// Exact stable identity pair for one attached-content application site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentApplicationSite {
    raw: ExprId,
    id: CheckedContentApplicationId,
}

impl CheckedContentApplicationSite {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        let raw = evidence.owner();
        let id = CheckedContentApplicationId::from_evidence(evidence);
        Self { raw, id }
    }

    pub const fn raw(&self) -> ExprId {
        self.raw
    }

    pub const fn id(&self) -> &CheckedContentApplicationId {
        &self.id
    }
}

/// Stable accepted identity of one attached-content application expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedContentApplicationId(CheckedSemanticPath);

impl CheckedContentApplicationId {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        Self(evidence.into_coordinate())
    }

    pub(crate) const fn path(&self) -> &CheckedSemanticPath {
        &self.0
    }

    /// Returns the stable root fragment coordinate for this exact checked
    /// content-producing expression. Downstream plans extend the typed path
    /// through `try_child` and never encode the private semantic path.
    pub fn fragment_coordinate(&self) -> StableCheckedContentFragmentCoordinate {
        StableCheckedContentFragmentCoordinate::root(StableCheckedValueCoordinate::Expression(
            self.0.clone(),
        ))
    }
}

/// Raw value expression paired with its accepted semantic path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentValueSource {
    raw: ExprId,
    path: CheckedSemanticPath,
}

impl CheckedContentValueSource {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        Self {
            raw: evidence.owner(),
            path: evidence.into_coordinate(),
        }
    }

    pub const fn raw(&self) -> ExprId {
        self.raw
    }

    pub const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub fn fragment_coordinate(&self) -> StableCheckedContentFragmentCoordinate {
        StableCheckedContentFragmentCoordinate::root(StableCheckedValueCoordinate::Expression(
            self.path.clone(),
        ))
    }
}

/// Checked content-site/body/emission/failure evidence.  Producer and runtime
/// ownership live in the enclosing final-analysis expression authority; this
/// token contains only the content-specific material needed by the renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentInsertion {
    site: CheckedContentApplicationSite,
    fragment_coordinate: StableCheckedContentFragmentCoordinate,
    argument: CheckedAttachedContentArgument,
    emission: CheckedContentEmission,
    failure_selection: InlineFailureSelection,
}

impl CheckedContentInsertion {
    pub(crate) fn new(
        site: CheckedContentApplicationSite,
        fragment_coordinate: StableCheckedContentFragmentCoordinate,
        argument: CheckedAttachedContentArgument,
        emission: CheckedContentEmission,
        failure_selection: InlineFailureSelection,
    ) -> Self {
        Self {
            site,
            fragment_coordinate,
            argument,
            emission,
            failure_selection,
        }
    }

    pub const fn site(&self) -> &CheckedContentApplicationSite {
        &self.site
    }

    /// Returns the report-local stable identity of the fragment containing
    /// this insertion. A `ContentResult` insertion owns a child-extended
    /// coordinate; structural insertions retain their enclosing fragment.
    pub const fn fragment_coordinate(&self) -> &StableCheckedContentFragmentCoordinate {
        &self.fragment_coordinate
    }

    pub const fn argument(&self) -> &CheckedAttachedContentArgument {
        &self.argument
    }

    pub const fn emission(&self) -> &CheckedContentEmission {
        &self.emission
    }

    pub const fn failure_selection(&self) -> &InlineFailureSelection {
        &self.failure_selection
    }
}

/// Ordered renderer-neutral dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueToken {
    Text(Box<str>),
    Escape(char),
    Interpolation(ExprId),
    PointAction(CheckedRichTextAction),
    /// A checked attached-content insertion. Producer and execution authority
    /// are carried by the owning final-analysis expression; this token keeps
    /// only the content-specific evidence consumed by rendering.
    ContentInsert(CheckedContentInsertion),
    LineBreak(HirLineBreakKind),
    /// Typed raw body. Its bytes were decoded by HIR and are never reparsed.
    RawLiteral(Box<str>),
}

/// Checked tokens and point actions for one final-HIR content owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueContent {
    id: HirDialogueContentId,
    tokens: Box<[CheckedDialogueToken]>,
    diagnostics_complete: bool,
}

impl CheckedDialogueContent {
    pub(crate) fn new(
        id: HirDialogueContentId,
        tokens: Vec<CheckedDialogueToken>,
        diagnostics_complete: bool,
    ) -> Self {
        Self {
            id,
            tokens: tokens.into_boxed_slice(),
            diagnostics_complete,
        }
    }

    pub const fn id(&self) -> HirDialogueContentId {
        self.id
    }

    pub const fn tokens(&self) -> &[CheckedDialogueToken] {
        &self.tokens
    }

    pub const fn diagnostics_complete(&self) -> bool {
        self.diagnostics_complete
    }
}

impl CheckedDialogueToken {
    /// Stable token-family tag used by the checked RichText transcript.
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Text(_) => 0x00,
            Self::Escape(_) => 0x01,
            Self::Interpolation(_) => 0x02,
            Self::PointAction(_) => 0x03,
            Self::ContentInsert(_) => 0x04,
            Self::LineBreak(_) => 0x05,
            Self::RawLiteral(_) => 0x06,
        }
    }
}

/// Typed validation report shared by semantic and rendering consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRichTextReport {
    admission: CheckedAttachedContentAdmission,
    fragment_coordinate: StableCheckedContentFragmentCoordinate,
    content: CheckedDialogueContent,
    diagnostics: Box<[RichTextDiagnostic]>,
    effect_plan: crate::final_analysis::CheckedDialogueEffectPlan,
}

impl CheckedRichTextReport {
    pub(crate) fn new(
        admission: CheckedAttachedContentAdmission,
        fragment_coordinate: StableCheckedContentFragmentCoordinate,
        content: CheckedDialogueContent,
        diagnostics: Vec<RichTextDiagnostic>,
    ) -> Self {
        Self {
            admission,
            fragment_coordinate,
            content,
            diagnostics: diagnostics.into_boxed_slice(),
            effect_plan: crate::final_analysis::CheckedDialogueEffectPlan::new([]),
        }
    }

    pub const fn admission(&self) -> CheckedAttachedContentAdmission {
        self.admission
    }

    /// Returns the stable coordinate issued by final sema for this checked
    /// fragment. Downstream plans consume this coordinate directly and never
    /// reconstruct the accepted semantic path from HIR IDs.
    pub const fn fragment_coordinate(&self) -> &StableCheckedContentFragmentCoordinate {
        &self.fragment_coordinate
    }

    pub(crate) fn with_effect_plan(
        mut self,
        effect_plan: crate::final_analysis::CheckedDialogueEffectPlan,
    ) -> Self {
        self.effect_plan = effect_plan;
        self
    }

    pub const fn content(&self) -> &CheckedDialogueContent {
        &self.content
    }

    pub const fn diagnostics(&self) -> &[RichTextDiagnostic] {
        &self.diagnostics
    }

    pub const fn effect_plan(&self) -> &crate::final_analysis::CheckedDialogueEffectPlan {
        &self.effect_plan
    }

    pub const fn is_valid(&self) -> bool {
        self.content.diagnostics_complete() && self.diagnostics.is_empty()
    }
}
