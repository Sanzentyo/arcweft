//! Facade crate for Arcweft application-facing model imports.
//!
//! Runtime code should depend on narrower crates directly. This crate exists for
//! examples, application code, and tests that want the common Arcweft prelude.

pub mod prelude {
    pub use arcweft_adt::{
        Array, BTreeMap, BTreeSet, BitSet, EventLog, Map, Never, OrderedMap, OrderedSet, Patch,
        Set, Snapshot, StableHash, TraceLog, Unit, Vec, VecDeque,
    };
    pub use arcweft_core::*;
    pub use arcweft_dialogue::{
        CancelAction, CancelOnDrop, CancelRule, CancelScope, CancelTrigger, Cue, CueAction,
        DialogueBuildError, DialogueBuildErrorKind, DialogueContent, DialogueContentPart,
        DialogueLine, DialogueLineBuilder, DialogueOptions, DialogueTag, InputEventKind, LineExit,
        LinePlan, LinePlanBuilder, LinePlanStep, OutPayload, PlanArg, PlanCall, PlanExpr,
        SayOptions, SpeakerPreset, SpeakerRef, TagArg, TextBoxRef, TimelineAnchor, TimelineCue,
        VoicePolicy, VoiceRef, character, line_id, textbox,
    };
    pub use arcweft_id::{EntityId, IdError, IdErrorKind, PublicId, TextKey};
    pub use arcweft_memory::{Blob, BlobRef, Bytes, MemoryLease, PodSlice, SharedSliceDesc};
    pub use arcweft_need::{Need, Progress, ProgressError};
    pub use arcweft_presentation::{
        BackgroundSurface, CharacterSurface, ClearPresentation, PresentationHandle,
        PresentationRegistry, PresentationScope, PresentationSlot, PresentationTarget, SlotRef,
        SlotValue, asset, bg, bg_ref, clear_bg, presentation_scope, presentation_slot,
        presentation_target,
    };
    pub use arcweft_ref::{Borrow, Handle, Id, Lease, Ref, Slice, WeakHandle};
    pub use arcweft_source::{SourceAnchor, SourceName, SourcePosition};
}
