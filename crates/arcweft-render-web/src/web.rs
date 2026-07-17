use arcweft_render_wgpu::geometry::PreparedFrame;
use arcweft_render_wgpu::renderer::{
    PreparedSharedRenderSubmission, SharedRenderer, SharedRendererError,
};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use winit::dpi::PhysicalSize;
use winit::window::Window;

/// Browser GPU health surfaced to the player and fatal-error bootstrap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebGpuHealth {
    pub device_lost: Option<String>,
    pub uncaptured_error: Option<String>,
}

/// WebGPU canvas/surface owner. The `Window` is retained so the surface has a
/// stable lifetime for the complete winit application.
pub struct WebGpuCanvasHost {
    window: Arc<dyn Window>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    configured: bool,
    health: Arc<Mutex<WebGpuHealth>>,
}

/// Acquired browser surface and fully encoded renderer work awaiting guarded
/// publication.
pub struct WebGpuCanvasFrameCandidate {
    surface_frame: wgpu::SurfaceTexture,
    submission: PreparedSharedRenderSubmission,
}

#[derive(Debug, Error)]
pub enum WebGpuCanvasHostError {
    #[error("WebGPU surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("no WebGPU adapter is available for the canvas surface")]
    AdapterUnavailable,
    #[error("WebGPU device acquisition failed: {0}")]
    DeviceRequest(String),
    #[error("the WebGPU surface reported no supported texture format")]
    NoSurfaceFormat,
    #[error("WebGPU canvas surface extent must be nonzero, got {width}x{height}")]
    InvalidSurfaceExtent { width: u32, height: u32 },
    #[error("WebGPU surface is out of memory")]
    OutOfMemory,
    #[error("WebGPU surface was lost")]
    SurfaceLost,
    #[error("WebGPU surface is outdated and must be reconfigured")]
    SurfaceOutdated,
    #[error("WebGPU surface acquisition timed out")]
    SurfaceTimeout,
    #[error("WebGPU surface is currently occluded")]
    SurfaceOccluded,
    #[error("WebGPU surface acquisition failed validation")]
    SurfaceValidation,
    #[error(transparent)]
    SharedRenderer(#[from] SharedRendererError),
}

impl WebGpuCanvasHost {
    /// Acquires a real WebGPU adapter/device. There is no WebGL or DOM game
    /// renderer fallback: failure returns a structured fatal error.
    pub async fn new(window: Arc<dyn Window>) -> Result<Self, WebGpuCanvasHostError> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| WebGpuCanvasHostError::SurfaceCreation(error.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_| WebGpuCanvasHostError::AdapterUnavailable)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arcweft-webgpu-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| WebGpuCanvasHostError::DeviceRequest(error.to_string()))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(WebGpuCanvasHostError::NoSurfaceFormat)?;
        let size = window.surface_size();
        let configured = size.width != 0 && size.height != 0;
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
        if configured {
            surface.configure(&device, &config);
        }
        let health = Arc::new(Mutex::new(WebGpuHealth::default()));
        install_device_callbacks(&device, &health);

        Ok(Self {
            window,
            surface,
            adapter,
            device,
            queue,
            config,
            configured,
            health,
        })
    }

    pub const fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub const fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub const fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    pub fn health(&self) -> WebGpuHealth {
        self.health.lock().map_or_else(
            |_| WebGpuHealth {
                device_lost: Some("WebGPU health lock was poisoned".to_owned()),
                uncaptured_error: None,
            },
            |health| health.clone(),
        )
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if self.config.width == size.width && self.config.height == size.height {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.configured = size.width != 0 && size.height != 0;
        if self.configured {
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn reconfigure(&mut self) {
        if self.configured {
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Acquires the canvas surface and completes every fallible render step
    /// without queue submission or presentation.
    pub fn prepare_frame(
        &mut self,
        renderer: &mut SharedRenderer,
        frame: &PreparedFrame,
    ) -> Result<WebGpuCanvasFrameCandidate, WebGpuCanvasHostError> {
        if !self.configured {
            return Err(WebGpuCanvasHostError::InvalidSurfaceExtent {
                width: self.config.width,
                height: self.config.height,
            });
        }
        let surface_frame =
            WebGpuCanvasHostError::surface_texture(self.surface.get_current_texture())?;
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let submission = renderer.prepare_to_view(&self.device, &self.queue, &view, frame)?;
        Ok(WebGpuCanvasFrameCandidate {
            surface_frame,
            submission,
        })
    }

    /// Infallibly submits and presents one previously prepared browser frame.
    pub fn commit_frame(&self, candidate: WebGpuCanvasFrameCandidate) {
        candidate.submission.submit(&self.queue);
        candidate.surface_frame.present();
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

fn install_device_callbacks(device: &wgpu::Device, health: &Arc<Mutex<WebGpuHealth>>) {
    let uncaptured = Arc::clone(health);
    device.on_uncaptured_error(Arc::new(move |error| {
        if let Ok(mut health) = uncaptured.lock() {
            health.uncaptured_error = Some(error.to_string());
        }
        web_sys::console::error_1(&format!("Arcweft WebGPU uncaptured error: {error}").into());
    }));

    let lost = Arc::clone(health);
    device.set_device_lost_callback(move |reason, message| {
        if let Ok(mut health) = lost.lock() {
            health.device_lost = Some(format!("{reason:?}: {message}"));
        }
        web_sys::console::error_1(
            &format!("Arcweft WebGPU device lost ({reason:?}): {message}").into(),
        );
    });
}

impl WebGpuCanvasHostError {
    fn surface_texture(status: wgpu::CurrentSurfaceTexture) -> Result<wgpu::SurfaceTexture, Self> {
        match status {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
            wgpu::CurrentSurfaceTexture::Timeout => Err(Self::SurfaceTimeout),
            wgpu::CurrentSurfaceTexture::Occluded => Err(Self::SurfaceOccluded),
            wgpu::CurrentSurfaceTexture::Outdated => Err(Self::SurfaceOutdated),
            wgpu::CurrentSurfaceTexture::Lost => Err(Self::SurfaceLost),
            wgpu::CurrentSurfaceTexture::Validation => Err(Self::SurfaceValidation),
        }
    }
}
