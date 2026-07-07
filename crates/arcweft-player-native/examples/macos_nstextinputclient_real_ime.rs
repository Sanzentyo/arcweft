#[cfg(not(all(target_os = "macos", feature = "macos-appkit-ime-sample")))]
fn main() {
    eprintln!(
        "macos_nstextinputclient_real_ime requires macOS and feature macos-appkit-ime-sample"
    );
}

#[cfg(all(target_os = "macos", feature = "macos-appkit-ime-sample"))]
fn main() -> std::process::ExitCode {
    match app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(target_os = "macos", feature = "macos-appkit-ime-sample"))]
mod app {
    use arcweft_desktop_native::text_input::macos_appkit_bridge::{
        MacosAppKitBridge, MacosAppKitBridgeEvent, MacosAppKitBridgeMode,
        MacosAppKitBridgeSpawnOptions, MacosAppKitBridgeState, MacosAppKitWireCharacterBounds,
        MacosAppKitWireRange, MacosAppKitWireRect,
    };
    use arcweft_desktop_native::text_input::macos_text_input::{
        MacosFirstRectAvailability, MacosNativeRange, MacosScreenCoordinateSpace,
        MacosTextInputAdapter,
    };
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{InputEpoch, InteractionTarget, RawInputKind};
    use arcweft_presentation::text_editor::{
        TextEditorGlyphGeometry, TextEditorLayout, TextEditorLayoutParts, TextEditorLayoutSource,
        TextEditorLocalClipboard, TextEditorState,
    };
    use arcweft_presentation::text_index::TextIndexSnapshot;
    use arcweft_presentation::text_input::{
        PlatformTextInputEvent, TextByteOffset, TextEditCommand, TextGeometryTransform,
        TextInputAdapterKind, TextInputBlurPolicy, TextInputCapabilities, TextInputFocusGeneration,
        TextInputKeyDisposition, TextInputSessionId, TextRange, TextWritingMode,
    };
    use arcweft_runtime_host::TextInputDispatchState;
    use std::env;

    const SAMPLE_SESSION: TextInputSessionId = TextInputSessionId(0x0604_c200);

    #[derive(Clone, Copy, Debug)]
    struct GeometryContext {
        screen_height_points: f64,
        view_origin_x: f32,
        view_origin_y: f32,
    }

    #[derive(Clone, Debug)]
    struct SampleArgs {
        mode: MacosAppKitBridgeMode,
        initial: String,
    }

    pub fn run() -> Result<(), String> {
        let args = SampleArgs::parse()?;
        let target = InteractionTarget::new(
            PublicId::try_new(format!(
                "sample.macos.nstextinputclient.{}",
                args.mode.as_str()
            ))
            .map_err(|error| error.to_string())?,
        );
        let mut geometry = GeometryContext::default();
        let mut editor = TextEditorState::new(
            SAMPLE_SESSION,
            target.clone(),
            args.initial,
            args.mode.options(),
        )
        .map_err(|error| error.to_string())?;
        let mut clipboard = TextEditorLocalClipboard::default();
        let mut dispatcher = TextInputDispatchState::default();
        let mut generation = activate_dispatcher(&mut dispatcher, &editor, &target, geometry)?;
        let mut adapter = activate_adapter(&editor, &target, generation, geometry)?;
        let mut bridge = MacosAppKitBridge::spawn(
            &MacosAppKitBridgeSpawnOptions::new(args.mode)
                .with_title(format!("Arcweft macOS IME {:?}", args.mode)),
        )
        .map_err(|error| error.to_string())?;
        send_state(
            &mut bridge,
            &mut adapter,
            &editor,
            args.mode,
            generation,
            geometry,
        )?;

        let mut epoch = 1_u64;
        let mut active = true;
        while let Some(event) = bridge.read_event().map_err(|error| error.to_string())? {
            match event {
                MacosAppKitBridgeEvent::Ready {
                    screen_height_points,
                    view_origin_x,
                    view_origin_y,
                }
                | MacosAppKitBridgeEvent::GeometryRefresh {
                    screen_height_points,
                    view_origin_x,
                    view_origin_y,
                } => {
                    geometry = GeometryContext {
                        screen_height_points,
                        view_origin_x: view_origin_x as f32,
                        view_origin_y: view_origin_y as f32,
                    };
                }
                MacosAppKitBridgeEvent::Focus => {
                    if !active {
                        generation =
                            activate_dispatcher(&mut dispatcher, &editor, &target, geometry)?;
                        adapter = activate_adapter(&editor, &target, generation, geometry)?;
                        active = true;
                    }
                }
                MacosAppKitBridgeEvent::Blur => {
                    if active {
                        dispatch_event(
                            &mut dispatcher,
                            &mut editor,
                            &mut clipboard,
                            &mut epoch,
                            adapter.cancel_marked_text().into_event(),
                        )?;
                        let _ = dispatcher.blur(TextInputBlurPolicy::CancelComposition);
                        active = false;
                    }
                }
                MacosAppKitBridgeEvent::SetMarkedText {
                    text,
                    selected_range,
                    replacement_range,
                } => {
                    ensure_active(active)?;
                    let event = adapter
                        .set_marked_text(text, selected_range.native(), replacement_range.native())
                        .map_err(|error| error.to_string())?
                        .into_event();
                    dispatch_event(
                        &mut dispatcher,
                        &mut editor,
                        &mut clipboard,
                        &mut epoch,
                        event,
                    )?;
                }
                MacosAppKitBridgeEvent::InsertText {
                    text,
                    replacement_range,
                } => {
                    ensure_active(active)?;
                    let event = adapter
                        .insert_text(text, replacement_range.native())
                        .map_err(|error| error.to_string())?
                        .into_event();
                    dispatch_event(
                        &mut dispatcher,
                        &mut editor,
                        &mut clipboard,
                        &mut epoch,
                        event,
                    )?;
                }
                MacosAppKitBridgeEvent::UnmarkText => {
                    ensure_active(active)?;
                    dispatch_event(
                        &mut dispatcher,
                        &mut editor,
                        &mut clipboard,
                        &mut epoch,
                        adapter.unmark_text().into_event(),
                    )?;
                }
                MacosAppKitBridgeEvent::Command { selector } => {
                    ensure_active(active)?;
                    if let Some(command) = selector_command(&selector) {
                        let event = adapter.command(command).into_event();
                        dispatch_event(
                            &mut dispatcher,
                            &mut editor,
                            &mut clipboard,
                            &mut epoch,
                            event,
                        )?;
                    }
                }
                MacosAppKitBridgeEvent::BridgeError { message } => {
                    return Err(format!("Swift AppKit bridge reported: {message}"));
                }
                MacosAppKitBridgeEvent::Exit => break,
            }
            if active {
                send_state(
                    &mut bridge,
                    &mut adapter,
                    &editor,
                    args.mode,
                    generation,
                    geometry,
                )?;
            }
        }
        Ok(())
    }

    fn ensure_active(active: bool) -> Result<(), String> {
        active
            .then_some(())
            .ok_or_else(|| "received AppKit text callback after focus deactivation".to_owned())
    }

    fn activate_dispatcher(
        dispatcher: &mut TextInputDispatchState,
        editor: &TextEditorState,
        target: &InteractionTarget,
        geometry: GeometryContext,
    ) -> Result<TextInputFocusGeneration, String> {
        let layout = renderer_layout(editor.text(), geometry)?;
        let snapshots = editor
            .snapshots(&layout)
            .map_err(|error| error.to_string())?;
        let generation = dispatcher
            .activate_with_capabilities(
                snapshots.client(),
                TextInputCapabilities::for_platform_adapter(
                    TextInputAdapterKind::MacosTextInputClient,
                ),
            )
            .generation();
        if snapshots.client().target() != target {
            return Err("activation target drifted while building macOS sample".to_owned());
        }
        Ok(generation)
    }

    fn activate_adapter(
        editor: &TextEditorState,
        _target: &InteractionTarget,
        generation: TextInputFocusGeneration,
        geometry: GeometryContext,
    ) -> Result<MacosTextInputAdapter, String> {
        let layout = renderer_layout(editor.text(), geometry)?;
        let snapshots = editor
            .snapshots(&layout)
            .map_err(|error| error.to_string())?;
        MacosTextInputAdapter::activate(snapshots.client(), generation)
            .map_err(|error| error.to_string())
    }

    fn dispatch_event(
        dispatcher: &mut TextInputDispatchState,
        editor: &mut TextEditorState,
        clipboard: &mut TextEditorLocalClipboard,
        epoch: &mut u64,
        event: PlatformTextInputEvent,
    ) -> Result<(), String> {
        let output = dispatcher
            .dispatch_platform_event(
                InputEpoch(*epoch),
                event,
                TextInputKeyDisposition::ImeConsumed,
            )
            .map_err(|error| error.to_string())?;
        *epoch = epoch.saturating_add(1);
        let raw = output.into_raw();
        let RawInputKind::Text(input) = raw.kind() else {
            return Err("runtime-host did not emit text input".to_owned());
        };
        editor
            .apply_text_input(input, clipboard)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn send_state(
        bridge: &mut MacosAppKitBridge,
        adapter: &mut MacosTextInputAdapter,
        editor: &TextEditorState,
        mode: MacosAppKitBridgeMode,
        generation: TextInputFocusGeneration,
        geometry: GeometryContext,
    ) -> Result<(), String> {
        let layout = renderer_layout(editor.text(), geometry)?;
        let snapshots = editor
            .snapshots(&layout)
            .map_err(|error| error.to_string())?;
        adapter
            .refresh_snapshot(snapshots.client(), generation)
            .map_err(|error| error.to_string())?;
        let screen = MacosScreenCoordinateSpace::top_left_screen(geometry.screen_height_points);
        let first_rect = adapter
            .first_rect_for_character_range(
                snapshots.geometry(),
                MacosNativeRange::not_found(),
                screen,
            )
            .map_err(|error| error.to_string())?;
        let secure = mode.is_secure();
        let selected_range = if secure {
            MacosAppKitWireRange::not_found()
        } else {
            adapter
                .selected_range_for_appkit()
                .map(MacosAppKitWireRange::from)
                .unwrap_or_else(|_| MacosAppKitWireRange::not_found())
        };
        let marked_range = if secure {
            MacosAppKitWireRange::not_found()
        } else {
            adapter
                .marked_range_for_appkit()
                .map(MacosAppKitWireRange::from)
                .unwrap_or_else(|_| MacosAppKitWireRange::not_found())
        };
        let character_bounds = if secure {
            Vec::new()
        } else {
            appkit_character_bounds(editor.text(), snapshots.geometry(), screen)?
        };
        bridge
            .write_state(&MacosAppKitBridgeState {
                session: SAMPLE_SESSION.0,
                revision: editor.revision().0,
                mode: mode.as_str().to_owned(),
                display_text: if secure {
                    String::new()
                } else {
                    editor.text().to_owned()
                },
                selected_range,
                marked_range,
                has_marked_text: !secure && adapter.has_marked_text_for_appkit(),
                first_rect: MacosAppKitWireRect::from(first_rect.rect()),
                actual_range: if first_rect.availability()
                    == MacosFirstRectAvailability::SecureRedacted
                {
                    MacosAppKitWireRange::not_found()
                } else {
                    MacosAppKitWireRange::from(first_rect.actual_range())
                },
                character_bounds,
                secure,
                diagnostics: if secure {
                    Vec::new()
                } else {
                    vec![format!("{:?}", adapter.diagnostics())]
                },
            })
            .map_err(|error| error.to_string())
    }

    fn appkit_character_bounds(
        text: &str,
        geometry: &arcweft_presentation::text_input::TextInputGeometrySnapshot,
        screen: MacosScreenCoordinateSpace,
    ) -> Result<Vec<MacosAppKitWireCharacterBounds>, String> {
        let index =
            TextIndexSnapshot::try_new(text.to_owned()).map_err(|error| error.to_string())?;
        geometry
            .screen_character_bounds()
            .iter()
            .map(|bound| {
                let utf16 = index
                    .utf16_range_from_byte(bound.range)
                    .map_err(|error| error.to_string())?;
                Ok(MacosAppKitWireCharacterBounds {
                    range: MacosAppKitWireRange::from(MacosNativeRange::new(
                        u64::from(utf16.start().0),
                        u64::from(utf16.end().0.saturating_sub(utf16.start().0)),
                    )),
                    rect: MacosAppKitWireRect::from(screen.rect_to_appkit(bound.bounds)),
                })
            })
            .collect()
    }

    fn renderer_layout(text: &str, geometry: GeometryContext) -> Result<TextEditorLayout, String> {
        let glyphs = glyphs_for_text(text);
        TextEditorLayout::from_renderer_parts_for_text(
            text,
            TextEditorLayoutParts {
                source: TextEditorLayoutSource::Renderer,
                text_local_control_rect: HitRect::new(0.0, 0.0, 680.0, 42.0),
                glyphs,
                caret_width: 1.0,
                writing_mode: TextWritingMode::HorizontalTb,
                text_local_to_viewport: TextGeometryTransform::identity(),
                viewport_to_screen: TextGeometryTransform::translation(
                    geometry.view_origin_x,
                    geometry.view_origin_y,
                ),
            },
        )
        .map_err(|error| format!("renderer layout fixture invalid: {error:?}"))
    }

    fn glyphs_for_text(text: &str) -> Vec<TextEditorGlyphGeometry> {
        let mut x = 0.0_f32;
        text.char_indices()
            .map(|(byte, character)| {
                let start = TextByteOffset(u32::try_from(byte).unwrap_or(u32::MAX));
                let end = TextByteOffset(
                    start
                        .0
                        .saturating_add(u32::try_from(character.len_utf8()).unwrap_or(u32::MAX)),
                );
                let width = if character.is_ascii() { 11.0 } else { 22.0 };
                let glyph = TextEditorGlyphGeometry::new(
                    TextRange::new(start, end),
                    HitRect::new(x, 0.0, width, 28.0),
                );
                x += width;
                glyph
            })
            .collect()
    }

    fn selector_command(selector: &str) -> Option<TextEditCommand> {
        match selector {
            "deleteBackward:" => Some(TextEditCommand::Backspace),
            "deleteForward:" => Some(TextEditCommand::Delete),
            "moveLeft:" => Some(TextEditCommand::MoveLeft { selecting: false }),
            "moveRight:" => Some(TextEditCommand::MoveRight { selecting: false }),
            "moveWordLeft:" => Some(TextEditCommand::MoveWordLeft { selecting: false }),
            "moveWordRight:" => Some(TextEditCommand::MoveWordRight { selecting: false }),
            "moveToBeginningOfLine:" => Some(TextEditCommand::MoveLineStart { selecting: false }),
            "moveToEndOfLine:" => Some(TextEditCommand::MoveLineEnd { selecting: false }),
            "pageUp:" => Some(TextEditCommand::MovePageUp { selecting: false }),
            "pageDown:" => Some(TextEditCommand::MovePageDown { selecting: false }),
            "moveLeftAndModifySelection:" => Some(TextEditCommand::MoveLeft { selecting: true }),
            "moveRightAndModifySelection:" => Some(TextEditCommand::MoveRight { selecting: true }),
            "moveWordLeftAndModifySelection:" => {
                Some(TextEditCommand::MoveWordLeft { selecting: true })
            }
            "moveWordRightAndModifySelection:" => {
                Some(TextEditCommand::MoveWordRight { selecting: true })
            }
            "moveToBeginningOfLineAndModifySelection:" => {
                Some(TextEditCommand::MoveLineStart { selecting: true })
            }
            "moveToEndOfLineAndModifySelection:" => {
                Some(TextEditCommand::MoveLineEnd { selecting: true })
            }
            "pageUpAndModifySelection:" => Some(TextEditCommand::MovePageUp { selecting: true }),
            "pageDownAndModifySelection:" => {
                Some(TextEditCommand::MovePageDown { selecting: true })
            }
            "selectAll:" => Some(TextEditCommand::SelectAll),
            "insertNewline:" => Some(TextEditCommand::Submit),
            "cancelOperation:" => Some(TextEditCommand::Cancel),
            _ => None,
        }
    }

    impl Default for GeometryContext {
        fn default() -> Self {
            Self {
                screen_height_points: 900.0,
                view_origin_x: 200.0,
                view_origin_y: 200.0,
            }
        }
    }

    impl SampleArgs {
        fn parse() -> Result<Self, String> {
            let mut mode = MacosAppKitBridgeMode::TextField;
            let mut initial = String::new();
            let mut args = env::args().skip(1);
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--mode" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "--mode requires a value".to_owned())?;
                        mode = match value.as_str() {
                            "text-field" => MacosAppKitBridgeMode::TextField,
                            "text-area" => MacosAppKitBridgeMode::TextArea,
                            "secure-field" => MacosAppKitBridgeMode::SecureField,
                            _ => return Err(format!("unknown mode {value}")),
                        };
                    }
                    "--initial" => {
                        initial = args
                            .next()
                            .ok_or_else(|| "--initial requires a value".to_owned())?;
                    }
                    _ => return Err(format!("unknown argument {arg}")),
                }
            }
            Ok(Self { mode, initial })
        }
    }
}
