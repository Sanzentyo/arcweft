use arcweft_desktop_native::text_input::windows_tsf::{
    TsfAcp, TsfAcpRange, TsfDisplayAttributeClass, TsfDisplayAttributeSegment, TsfLayoutResult,
    TsfRangeError, TsfScreenRect, TsfTextSnapshot, WindowsTsfAdapter, WindowsTsfEditAccess,
    WindowsTsfEventContext, WindowsTsfFeature, WindowsTsfFeatureStatus, WindowsTsfGeometry,
    WindowsTsfRuntimeFacts, WindowsTsfSerialAllocator,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextByteOffset, TextCharacterBounds, TextCommit,
    TextCompositionSegmentKind, TextGeometryTransform, TextInputClientSnapshot,
    TextInputFocusGeneration, TextInputGeometrySnapshot, TextInputGeometrySnapshotParts,
    TextInputOperation, TextInputOptions, TextInputSecurityPolicy, TextInputSerial,
    TextInputSessionId, TextRange, TextRevision, TextWritingMode,
};

fn target() -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new("textfield.windows_tsf").unwrap())
}

fn snapshot(session: TextInputSessionId) -> TextInputClientSnapshot {
    TextInputClientSnapshot::new(
        session,
        target(),
        TextRevision(7),
        "a🦀b",
        TextByteOffset(0),
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        HitRect::new(10.0, 20.0, 200.0, 24.0),
        HitRect::new(11.0, 20.0, 1.0, 24.0),
        TextInputOptions::default(),
    )
}

#[test]
fn acp_range_around_surrogate_pair_maps_to_canonical_bytes() {
    let tsf = TsfTextSnapshot::plain(TextRevision(7), "a🦀b");
    let range = TsfAcpRange::new(TsfAcp(1), TsfAcp(3));

    assert_eq!(
        range.to_canonical_byte_range(&tsf, TextRevision(7)),
        Ok(TextRange::new(TextByteOffset(1), TextByteOffset(5)))
    );
}

#[test]
fn invalid_acp_range_rejects_without_clamp() {
    let tsf = TsfTextSnapshot::plain(TextRevision(1), "abc");
    let range = TsfAcpRange::new(TsfAcp(3), TsfAcp(1));

    assert_eq!(
        range.to_canonical_byte_range(&tsf, TextRevision(1)),
        Err(TsfRangeError::Unordered { start: 3, end: 1 })
    );
}

#[test]
fn edit_session_groups_operations_under_one_serial() {
    let mut serials = WindowsTsfSerialAllocator::new(TextInputSerial(10));
    let base =
        WindowsTsfEventContext::new(TextInputSessionId(2), TextInputFocusGeneration(5), target());
    let event = serials
        .begin_session(base, WindowsTsfEditAccess::ReadWrite)
        .with_operation(TextInputOperation::StartComposition)
        .with_operation(TextInputOperation::Commit(TextCommit::new("日本語")))
        .finish()
        .unwrap();

    let PlatformTextInputEvent::Batch {
        context,
        operations,
    } = event
    else {
        panic!("edit session must finish as a batch");
    };
    assert_eq!(context.serial(), TextInputSerial(10));
    assert_eq!(operations.len(), 2);
    assert_eq!(serials.peek(), TextInputSerial(11));
}

#[test]
fn reconversion_requires_runtime_function_fact() {
    let available = WindowsTsfRuntimeFacts::default()
        .with_runtime_ready()
        .with_reconversion_function_available();
    let unavailable = WindowsTsfRuntimeFacts::default().with_runtime_ready();

    let (_, activation) = WindowsTsfAdapter::activate(available);
    assert_eq!(
        activation
            .capabilities()
            .status(WindowsTsfFeature::Reconversion),
        WindowsTsfFeatureStatus::Supported
    );
    let (_, activation) = WindowsTsfAdapter::activate(unavailable);
    assert_eq!(
        activation
            .capabilities()
            .status(WindowsTsfFeature::Reconversion),
        WindowsTsfFeatureStatus::RuntimeUnavailable("tsf_reconversion_function_missing")
    );
}

#[test]
fn display_attributes_map_to_composition_segments() {
    let segment = TsfDisplayAttributeSegment::new(
        TextRange::new(TextByteOffset(0), TextByteOffset(6)),
        TsfDisplayAttributeClass::TargetConverted,
    )
    .to_composition_segment()
    .unwrap();

    assert_eq!(segment.kind(), TextCompositionSegmentKind::TargetConverted);
    assert_eq!(TsfDisplayAttributeClass::Other.composition_kind(), None);
}

#[test]
fn geometry_reports_clipped_no_layout_and_secure_redaction() {
    let geometry = TextInputGeometrySnapshot::new(TextInputGeometrySnapshotParts {
        session: TextInputSessionId(9),
        revision: TextRevision(4),
        writing_mode: TextWritingMode::default(),
        text_local_control_rect: HitRect::new(0.0, 0.0, 100.0, 20.0),
        text_local_caret_rect: HitRect::new(5.25, 0.0, 1.0, 20.0),
        text_local_character_bounds: vec![TextCharacterBounds::new(
            TextRange::new(TextByteOffset(0), TextByteOffset(1)),
            HitRect::new(5.25, 0.0, 8.25, 20.0),
        )],
        text_local_selection_rects: Vec::new(),
        text_local_composition_rects: Vec::new(),
        text_local_to_viewport: TextGeometryTransform::default(),
        viewport_to_screen: TextGeometryTransform::default(),
    });
    let plain = WindowsTsfGeometry::new(TextInputSecurityPolicy::Plain);
    assert_eq!(
        plain.text_ext(
            &geometry,
            TextRevision(4),
            TextRange::new(TextByteOffset(0), TextByteOffset(1)),
            true,
        ),
        TsfLayoutResult::Available {
            rect: TsfScreenRect::new(5, 0, 14, 20),
            clipped: true,
        }
    );
    assert_eq!(
        plain.text_ext(
            &geometry,
            TextRevision(4),
            TextRange::new(TextByteOffset(1), TextByteOffset(2)),
            false,
        ),
        TsfLayoutResult::NoLayout
    );
    let secure = WindowsTsfGeometry::new(TextInputSecurityPolicy::SecureRedacted);
    assert_eq!(
        secure.text_ext(
            &geometry,
            TextRevision(4),
            TextRange::new(TextByteOffset(0), TextByteOffset(1)),
            false,
        ),
        TsfLayoutResult::SecureRedacted
    );
}

#[test]
fn delayed_event_after_focus_generation_keeps_old_generation_for_runtime_rejection() {
    let mut adapter =
        WindowsTsfAdapter::activate(WindowsTsfRuntimeFacts::default().with_runtime_ready())
            .0
            .with_first_serial(TextInputSerial(1));
    let event = adapter
        .begin_edit_session(
            &snapshot(TextInputSessionId(12)),
            TextInputFocusGeneration(2),
            WindowsTsfEditAccess::CommandCallback,
        )
        .with_operation(TextInputOperation::Commit(TextCommit::new("x")))
        .finish()
        .unwrap();

    assert_eq!(event.context().generation(), TextInputFocusGeneration(2));
}

#[test]
fn secure_snapshot_rejects_acp_conversion_as_secure_redacted() {
    let tsf = TsfTextSnapshot::secure_redacted(TextRevision(3));

    assert_eq!(
        TsfAcpRange::new(TsfAcp(0), TsfAcp(1)).to_canonical_byte_range(&tsf, TextRevision(3)),
        Err(TsfRangeError::SecureRedacted)
    );
}
