//! Window-owned frame preparation, rendering, and boundary update order.

use super::{
    Arc, ArcweftBundle, BundleStepInput, FrameBoundary, Instant, ModifiersState,
    NativeAudioRuntime, NativeClipboardAdapter, NativeDesktopBackend, NativePlayerOptions,
    NativeSceneState, NativeSceneWindowError, NativeTextInputBridge, NativeTextInputFocusReason,
    PhysicalSize, PlayerFontSet, PlayerFrameError, PlayerFramePlannerState, PlayerFrameRequest,
    PlayerPreparedFrame, PlayerPreparedFrameCandidate, PreparedSharedRenderSubmission,
    RenderPreferences, RenderViewport, RuntimeClockStep, SharedRenderer, Size, VecDeque,
    ViewGeometryConsumer, ViewGeometryConversionError, ViewGeometryConversionField,
    ViewGeometryPlatform, ViewGeometryRuntimeError, Window, WindowCloseSignal,
    WindowedEnvironmentIngressCommand, WindowedEnvironmentIngressCompletion,
    WindowedEnvironmentUpdateError, WindowedPatchIngressCompletion, WindowedPatchIngressMessage,
    WindowedRuntimeOutcome, WinitOwnedWindowDriver, restored_windowed_runtime_and_input,
    save_native_player_session, scene_aspect_size, surface_texture,
};
use arcweft_render_wgpu::geometry::view_final::PreparedViewRenderCandidate;

struct NativeFramePublicationCandidate {
    view_render: PreparedViewRenderCandidate,
    surface_frame: wgpu::SurfaceTexture,
    submission: PreparedSharedRenderSubmission,
}

impl NativeFramePublicationCandidate {
    fn commit(self, queue: &wgpu::Queue, frame: &PlayerPreparedFrame) {
        debug_assert_eq!(
            self.view_render.generation(),
            frame.view_geometry().generation().value()
        );
        self.submission.submit(queue);
        self.surface_frame.present();
    }
}

impl NativeSceneState {
    #[expect(
        clippy::too_many_lines,
        reason = "native initialization acquires and validates one mutually dependent window, surface, device, renderer, session, and publication state"
    )]
    pub(super) async fn new(
        window: Arc<dyn Window>,
        title: String,
        bundle: ArcweftBundle,
        ingress_completion: WindowedPatchIngressCompletion,
        environment_completion: WindowedEnvironmentIngressCompletion,
        options: NativePlayerOptions,
    ) -> Result<Self, NativeSceneWindowError> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| NativeSceneWindowError::SurfaceCreation(error.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_| NativeSceneWindowError::AdapterUnavailable)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arcweft-native-scene-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| NativeSceneWindowError::DeviceRequest(error.to_string()))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(NativeSceneWindowError::NoSurfaceFormat)?;
        let size = scene_aspect_size(window.surface_size(), options.frame_fit)?;
        let surface_configured = size.width != 0 && size.height != 0;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: capabilities
                .present_modes
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::PresentMode::Fifo)
                .unwrap_or(wgpu::PresentMode::AutoVsync),
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: Vec::new(),
        };
        if surface_configured {
            surface.configure(&device, &config);
        }
        let mut renderer = SharedRenderer::new(&device, &queue, format);
        let mut frame_planner = PlayerFramePlannerState::new();
        PlayerFontSet::bundled_default()
            .register_with_renderer_and_planner(&mut renderer, &mut frame_planner)
            .map_err(|error| NativeSceneWindowError::Font(error.to_string()))?;
        let close_signal = WindowCloseSignal::default();
        let owned_window = Arc::new(
            WinitOwnedWindowDriver::try_new(Arc::clone(&window), title, close_signal.clone())
                .map_err(NativeSceneWindowError::Window)?,
        );
        let backend = NativeDesktopBackend::builder()
            .with_owned_window_driver(owned_window)
            .build();
        let audio = NativeAudioRuntime::from_bundle(&bundle)?;
        let (runtime, input, dialogue_visual_clock) = restored_windowed_runtime_and_input(
            &bundle,
            backend,
            options.session_load.as_deref(),
            options.entry.as_ref(),
        )?;
        let text_input = NativeTextInputBridge::new(options.text_input.clone());
        Ok(Self {
            window,
            close_signal,
            surface,
            surface_configured,
            device,
            queue,
            config,
            renderer,
            frame_planner,
            runtime,
            audio,
            ingress_completion,
            environment_completion,
            input,
            clipboard: NativeClipboardAdapter::default(),
            keyboard_modifiers: ModifiersState::default(),
            text_input,
            window_ime_supported: true,
            window_ime_enabled: false,
            next_window_ime_serial: 1,
            frame_fit: options.frame_fit,
            session_save_out: options.session_save_out.clone(),
            session_save_on_exit_completed: false,
            prepared: None,
            pending_environment: VecDeque::new(),
            dialogue_visual_clock,
            started_at: Instant::now(),
            next_tick: 1,
        })
    }

    pub(super) fn take_close_requested(&self) -> bool {
        self.close_signal.take()
    }

    pub(super) fn save_session_on_exit(&mut self) -> Result<(), NativeSceneWindowError> {
        if self.session_save_on_exit_completed {
            return Ok(());
        }
        let Some(path) = self.session_save_out.clone() else {
            return Ok(());
        };
        save_native_player_session(
            &path,
            &self.runtime,
            &self.input,
            &self.dialogue_visual_clock,
            self.elapsed_millis(),
        )?;
        self.session_save_on_exit_completed = true;
        Ok(())
    }

    pub(super) fn resize(
        &mut self,
        requested: PhysicalSize<u32>,
    ) -> Result<(), NativeSceneWindowError> {
        let size = scene_aspect_size(requested, self.frame_fit)?;
        if size != requested {
            let _ = self.window.request_surface_size(Size::Physical(size));
        }
        if self.config.width == size.width && self.config.height == size.height {
            return Ok(());
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface_configured = size.width != 0 && size.height != 0;
        if self.surface_configured {
            self.surface.configure(&self.device, &self.config);
        }
        Ok(())
    }

    pub(super) fn redraw(&mut self) -> Result<(), NativeSceneWindowError> {
        self.runtime.pump_main_thread()?;
        self.step_runtime()?;
        let candidate = self.prepare_frame_candidate()?;
        self.sync_text_input_bridge(
            candidate.prepared(),
            NativeTextInputFocusReason::RedrawRefresh,
        )?;
        self.sync_window_ime(candidate.prepared());
        if !self.surface_configured {
            let (prepared, ()) = self.frame_planner.publication_guard().publish_with(
                candidate,
                &mut self.input,
                |_| (),
            )?;
            let patch_outcomes = self.drain_patch_events_after_render_submitted()?;
            let environment_invalidated = self.drain_environment_boundary();
            if environment_invalidated
                || patch_outcomes
                    .iter()
                    .any(WindowedRuntimeOutcome::invalidates_prepared_frame)
            {
                self.prepared = None;
            } else {
                self.prepared = Some(prepared.frame);
            }
            return Ok(());
        }
        let native_candidate = match self.prepare_native_publication(&candidate) {
            Ok(candidate) => candidate,
            Err(NativeSceneWindowError::SurfaceLost | NativeSceneWindowError::SurfaceOutdated) => {
                if self.surface_configured {
                    self.surface.configure(&self.device, &self.config);
                }
                self.window.request_redraw();
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let (prepared, ()) = self.frame_planner.publication_guard().publish_with(
            candidate,
            &mut self.input,
            |frame| native_candidate.commit(&self.queue, frame),
        )?;
        let patch_outcomes = self.drain_patch_events_after_render_submitted()?;
        let environment_invalidated = self.drain_environment_boundary();
        if environment_invalidated
            || patch_outcomes
                .iter()
                .any(WindowedRuntimeOutcome::invalidates_prepared_frame)
        {
            self.prepared = None;
        } else {
            self.prepared = Some(prepared.frame);
        }
        Ok(())
    }

    fn drain_environment_boundary(&mut self) -> bool {
        let mut invalidated = false;
        while let Some(envelope) = self.pending_environment.pop_front() {
            let sequence = envelope.sequence();
            let command = envelope.command();
            let result = match command {
                WindowedEnvironmentIngressCommand::ReplaceProvider(values) => self
                    .runtime
                    .session_mut()
                    .update_presentation_environment_provider(values),
                WindowedEnvironmentIngressCommand::ClearProvider => self
                    .runtime
                    .session_mut()
                    .clear_presentation_environment_provider(),
            }
            .map_err(WindowedEnvironmentUpdateError::from)
            .and_then(|update| {
                let player = self
                    .frame_planner
                    .apply_environment_update(update)
                    .map_err(WindowedEnvironmentUpdateError::from)?;
                if player.prepared_work_discarded() {
                    self.prepared = None;
                }
                invalidated |= player.prepared_work_discarded() || player.redraw_requested();
                Ok(update)
            });
            self.environment_completion
                .completed_at_frame_boundary(sequence, command, &result);
            envelope.complete(result);
        }
        if invalidated {
            self.window.request_redraw();
        }
        invalidated
    }

    fn step_runtime(&mut self) -> Result<(), NativeSceneWindowError> {
        if self.runtime.session().is_finished() {
            return Ok(());
        }
        if let Some(audio) = &mut self.audio {
            let mut events = Vec::new();
            audio.drain_events(&mut events);
            self.runtime.push_audio_events(events);
        }
        let clock = RuntimeClockStep::from_millis(self.next_tick, 16)?;
        self.next_tick = self.next_tick.saturating_add(1);
        let step = self
            .runtime
            .step_with_clock(clock, BundleStepInput::default());
        if let Some(audio) = &mut self.audio {
            let mut command_events = Vec::new();
            audio.submit_commands(step.audio_commands, &mut command_events);
            self.runtime.push_audio_events(command_events);
        }
        Ok(())
    }

    pub(super) fn apply_ingress_message(&mut self, message: WindowedPatchIngressMessage) {
        match message {
            WindowedPatchIngressMessage::Enqueue(envelope) => {
                let source = envelope.event.source();
                self.ingress_completion
                    .accepted_by_event_loop(envelope.sequence, source);
                self.runtime.push_patch_event(envelope.event);
            }
            WindowedPatchIngressMessage::RetainRejected { source, message } => {
                self.runtime.retain_patch_ingress_rejection(source, message);
            }
        }
        self.window.request_redraw();
    }

    fn prepare_frame_candidate(
        &mut self,
    ) -> Result<PlayerPreparedFrameCandidate, NativeSceneWindowError> {
        let viewport = self.viewport()?;
        let elapsed = self.elapsed_millis();
        let session = self.runtime.session();
        let presentation = session.presentation();
        let fx_definitions = session.fx_definitions();
        let dialogue_visual = self.dialogue_visual_clock.progress(
            presentation.dialogue.latest_active(),
            elapsed,
            None,
        );
        let style_environment = session.presentation_environment();
        Ok(self.frame_planner.prepare_candidate(
            &self.input,
            PlayerFrameRequest {
                presentation,
                fx_definitions,
                images: self.runtime.images(),
                style_program: session.view_style_program(),
                style_environment: &style_environment,
                style_palettes: session.view_style_palettes(),
                viewport,
                fit: self.frame_fit,
                image_time_millis: elapsed,
                visual_time_millis: dialogue_visual.elapsed_millis(),
                dialogue_reveal_complete: dialogue_visual.is_complete(),
                preferences: RenderPreferences::default(),
            },
        )?)
    }

    pub(super) fn prepare_frame(
        &mut self,
    ) -> Result<arcweft_player_scene::frame::PlayerPreparedFrame, NativeSceneWindowError> {
        let candidate = self.prepare_frame_candidate()?;
        self.frame_planner
            .publication_guard()
            .publish_with(candidate, &mut self.input, |_| ())
            .map(|(frame, ())| frame)
            .map_err(Into::into)
    }

    fn prepare_native_publication(
        &mut self,
        candidate: &PlayerPreparedFrameCandidate,
    ) -> Result<NativeFramePublicationCandidate, NativeSceneWindowError> {
        if !self.surface_configured {
            return Err(NativeSceneWindowError::InvalidSurfaceExtent {
                width: self.config.width,
                height: self.config.height,
            });
        }
        let view_render = PreparedViewRenderCandidate::prepare(
            candidate.view_geometry().generation().value(),
            candidate
                .view_geometry()
                .final_nodes()
                .map(|(_, geometry)| geometry),
        )
        .map_err(ViewGeometryRuntimeError::from)
        .map_err(PlayerFrameError::from)?;
        let surface_frame = surface_texture(self.surface.get_current_texture())?;
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let submission = self.renderer.prepare_to_view(
            &self.device,
            &self.queue,
            &view,
            candidate.prepared(),
        )?;
        Ok(NativeFramePublicationCandidate {
            view_render,
            surface_frame,
            submission,
        })
    }

    fn drain_patch_events_after_render_submitted(
        &mut self,
    ) -> Result<Vec<WindowedRuntimeOutcome>, NativeSceneWindowError> {
        let outcomes = self
            .runtime
            .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)?;
        self.ingress_completion
            .completed_at_frame_boundary(outcomes.len());
        if !outcomes.is_empty() {
            self.window.request_redraw();
        }
        Ok(outcomes)
    }

    fn viewport(&self) -> Result<RenderViewport, NativeSceneWindowError> {
        let size = PhysicalSize::new(self.config.width, self.config.height);
        let scale_factor = self.window.scale_factor();
        ViewGeometryConversionError::scale_factor(ViewGeometryPlatform::Native, scale_factor)?;
        ViewGeometryConversionError::exact_f32(
            None,
            ViewGeometryPlatform::Native,
            ViewGeometryConsumer::Layout,
            ViewGeometryConversionField::ViewportWidth,
            i64::from(size.width) * 1_000,
        )?;
        ViewGeometryConversionError::exact_f32(
            None,
            ViewGeometryPlatform::Native,
            ViewGeometryConsumer::Layout,
            ViewGeometryConversionField::ViewportHeight,
            i64::from(size.height) * 1_000,
        )?;
        let logical = ViewGeometryConversionError::viewport_input(
            ViewGeometryPlatform::Native,
            f64::from(size.width) / scale_factor,
            f64::from(size.height) / scale_factor,
        )?;
        Ok(RenderViewport {
            logical_width: ViewGeometryConversionError::exact_f32(
                None,
                ViewGeometryPlatform::Native,
                ViewGeometryConsumer::Layout,
                ViewGeometryConversionField::ViewportWidth,
                i64::from(logical.rect.right_milli),
            )?,
            logical_height: ViewGeometryConversionError::exact_f32(
                None,
                ViewGeometryPlatform::Native,
                ViewGeometryConsumer::Layout,
                ViewGeometryConversionField::ViewportHeight,
                i64::from(logical.rect.bottom_milli),
            )?,
            physical_width: size.width,
            physical_height: size.height,
            scale_factor,
        })
    }

    pub(super) fn elapsed_millis(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}
