use super::*;

impl BrowserWebGpuMathContext {
    pub fn availability() -> BrowserWebGpuAvailability {
        let global = js_sys::global();
        let secure_context = bool_property(&global, "isSecureContext");
        let cross_origin_isolated = bool_property(&global, "crossOriginIsolated");
        let navigator_gpu_present = js_sys::Reflect::get(&global, &"navigator".into())
            .ok()
            .and_then(|navigator| js_sys::Reflect::get(&navigator, &"gpu".into()).ok())
            .is_some_and(|gpu| !gpu.is_undefined() && !gpu.is_null());
        BrowserWebGpuAvailability {
            secure_context,
            navigator_gpu_present,
            cross_origin_isolated,
        }
    }

    /// Create a browser WebGPU context. This must be awaited by the browser
    /// adapter before dispatching kernels.
    pub async fn new() -> Result<Self, BrowserWebGpuError> {
        let availability = Self::availability();
        if !availability.secure_context {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::InsecureContext,
                "browser WebGPU requires a secure context",
            ));
        }
        if !availability.navigator_gpu_present {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::NavigatorGpuMissing,
                "navigator.gpu is unavailable",
            ));
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::AdapterUnavailable,
                    error.to_string(),
                )
            })?;
        let limits = BrowserWebGpuLimits::from(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arcweft-browser-runtime-math"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::DeviceRequestFailed,
                    error.to_string(),
                )
            })?;
        let device_lost = Arc::new(Mutex::new(None));
        let uncaptured_error = Arc::new(Mutex::new(None));
        let device_lost_callback = Arc::clone(&device_lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = device_lost_callback.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        let uncaptured_error_callback = Arc::clone(&uncaptured_error);
        device.on_uncaptured_error(Arc::new(move |error| {
            if let Ok(mut slot) = uncaptured_error_callback.lock() {
                *slot = Some(error.to_string());
            }
        }));
        let matmul_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-browser-matmul-f32"),
            source: wgpu::ShaderSource::Wgsl(MATMUL_SHADER.into()),
        });
        let add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-browser-add-f32"),
            source: wgpu::ShaderSource::Wgsl(ADD_SHADER.into()),
        });
        let bias_add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-browser-bias-add-f32"),
            source: wgpu::ShaderSource::Wgsl(BIAS_ADD_SHADER.into()),
        });
        let matmul_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-browser-matmul-f32"),
            layout: None,
            module: &matmul_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-browser-add-f32"),
            layout: None,
            module: &add_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let bias_add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-browser-bias-add-f32"),
            layout: None,
            module: &bias_add_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Ok(Self {
            device,
            queue,
            matmul_pipeline,
            add_pipeline,
            bias_add_pipeline,
            readback: None,
            async_readback: None,
            limits,
            device_lost,
            uncaptured_error,
            stats: BrowserWebGpuMathStats::default(),
            in_flight: 0,
        })
    }

    pub const fn limits(&self) -> BrowserWebGpuLimits {
        self.limits
    }

    pub const fn stats(&self) -> BrowserWebGpuMathStats {
        self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = BrowserWebGpuMathStats::default();
        self.in_flight = 0;
    }

    pub async fn auto_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        policy: BrowserWebGpuMathAutoPolicy,
    ) -> Result<BrowserWebGpuAutoMathResult<DenseMatrixF32>, BrowserWebGpuError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        let selection = policy.select_matmul_f32(lhs.rows(), lhs.cols(), rhs.cols(), self.limits);
        let value = match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => lhs.matmul_scalar(rhs)?,
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                    rows: lhs.rows(),
                    shared: lhs.cols(),
                    cols: rhs.cols(),
                });
                let prepared = self.prepare_matmul_f32(capacity)?;
                self.upload_prepared_matmul_f32(
                    &prepared,
                    lhs.values(),
                    rhs.values(),
                    lhs.rows(),
                    lhs.cols(),
                    rhs.cols(),
                )?;
                let submitted =
                    self.submit_resident_matmul_f32(&prepared, lhs.rows(), rhs.cols())?;
                let mut out = vec![0.0; lhs.rows() * rhs.cols()];
                self.read_submitted_f32(submitted, &mut out).await?;
                DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?
            }
        };
        Ok(BrowserWebGpuAutoMathResult {
            value,
            selection,
            stats: self.stats,
        })
    }

    pub async fn auto_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        policy: BrowserWebGpuMathAutoPolicy,
    ) -> Result<BrowserWebGpuAutoMathResult<DenseMatrixF32>, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        let selection = policy.select_elementwise_f32(lhs.values().len(), self.limits);
        let value = match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => lhs.add_scalar(rhs)?,
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let prepared = self.prepare_elementwise_f32(lhs.values().len())?;
                self.upload_prepared_elementwise_f32(&prepared, lhs.values(), rhs.values())?;
                let submitted =
                    self.submit_resident_elementwise_f32(&prepared, lhs.values().len())?;
                let mut out = vec![0.0; lhs.values().len()];
                self.read_submitted_f32(submitted, &mut out).await?;
                DenseMatrixF32::new(lhs.rows(), lhs.cols(), out)?
            }
        };
        Ok(BrowserWebGpuAutoMathResult {
            value,
            selection,
            stats: self.stats,
        })
    }

    pub async fn auto_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
        policy: BrowserWebGpuMathAutoPolicy,
    ) -> Result<BrowserWebGpuAutoMathResult<DenseTensorF32>, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        let selection = policy.select_elementwise_f32(lhs.values().len(), self.limits);
        let value = match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => lhs.add_scalar(rhs)?,
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let prepared = self.prepare_elementwise_f32(lhs.values().len())?;
                self.upload_prepared_elementwise_f32(&prepared, lhs.values(), rhs.values())?;
                let submitted =
                    self.submit_resident_elementwise_f32(&prepared, lhs.values().len())?;
                let mut out = vec![0.0; lhs.values().len()];
                self.read_submitted_f32(submitted, &mut out).await?;
                DenseTensorF32::new(lhs.shape().dims().to_vec(), out)?
            }
        };
        Ok(BrowserWebGpuAutoMathResult {
            value,
            selection,
            stats: self.stats,
        })
    }

    pub async fn matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, BrowserWebGpuError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        let out_len = checked_len_mul(lhs.rows(), rhs.cols())?;
        validate_f32_storage(self.limits, lhs.values().len())?;
        validate_f32_storage(self.limits, rhs.values().len())?;
        validate_f32_storage(self.limits, out_len)?;
        let mut out = vec![0.0; out_len];
        let params = MatmulParams {
            rows: checked_u32(lhs.rows())?,
            shared: checked_u32(lhs.cols())?,
            cols: checked_u32(rhs.cols())?,
            _pad: 0,
        };
        let pipeline = self.matmul_pipeline.clone();
        self.dispatch(
            &pipeline,
            &[
                bytemuck::cast_slice(lhs.values()),
                bytemuck::cast_slice(rhs.values()),
            ],
            bytemuck::bytes_of(&params),
            &mut out,
            checked_workgroups(self.limits, rhs.cols().div_ceil(16))?,
            checked_workgroups(self.limits, lhs.rows().div_ceil(16))?,
        )
        .await?;
        DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into)
    }

    pub async fn matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        let mut out = vec![0.0; lhs.values().len()];
        self.dispatch_add(lhs.values(), rhs.values(), &mut out)
            .await?;
        DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into)
    }

    pub async fn tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        let mut out = vec![0.0; lhs.values().len()];
        self.dispatch_add(lhs.values(), rhs.values(), &mut out)
            .await?;
        DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into)
    }

    pub fn prepare_elementwise_f32(
        &mut self,
        capacity_len: usize,
    ) -> Result<BrowserPreparedElementwiseF32, BrowserWebGpuError> {
        validate_f32_storage(self.limits, capacity_len)?;
        let byte_len = checked_f32_bytes(capacity_len)?.max(std::mem::size_of::<f32>() as u64);
        let lhs = self.storage_buffer_capacity(byte_len, "arcweft-browser-prepared-add-lhs");
        let rhs = self.storage_buffer_capacity(byte_len, "arcweft-browser-prepared-add-rhs");
        let out = self.output_buffer_capacity(byte_len, "arcweft-browser-prepared-add-out");
        let params = self.storage_buffer_capacity(
            std::mem::size_of::<AddParams>() as u64,
            "arcweft-browser-prepared-add-params",
        );
        let layout = self.add_pipeline.get_bind_group_layout(0);
        let entries = bind_group_entries(&lhs, &rhs, &out, &params);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-browser-prepared-add-bind-group"),
            layout: &layout,
            entries: &entries,
        });
        self.stats.buffer_creations += 4;
        self.stats.bind_group_rebuilds += 1;
        Ok(BrowserPreparedElementwiseF32 {
            capacity_len,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn prepare_matmul_f32(
        &mut self,
        capacity: BrowserMatmulCapacity,
    ) -> Result<BrowserPreparedMatmulF32, BrowserWebGpuError> {
        let lhs_len = checked_len_mul(capacity.rows, capacity.shared)?;
        let rhs_len = checked_len_mul(capacity.shared, capacity.cols)?;
        let out_len = checked_len_mul(capacity.rows, capacity.cols)?;
        validate_f32_storage(self.limits, lhs_len)?;
        validate_f32_storage(self.limits, rhs_len)?;
        validate_f32_storage(self.limits, out_len)?;
        let lhs = self.storage_buffer_capacity(
            checked_f32_bytes(lhs_len)?.max(std::mem::size_of::<f32>() as u64),
            "arcweft-browser-prepared-matmul-lhs",
        );
        let rhs = self.storage_buffer_capacity(
            checked_f32_bytes(rhs_len)?.max(std::mem::size_of::<f32>() as u64),
            "arcweft-browser-prepared-matmul-rhs",
        );
        let out = self.output_buffer_capacity(
            checked_f32_bytes(out_len)?.max(std::mem::size_of::<f32>() as u64),
            "arcweft-browser-prepared-matmul-out",
        );
        let params = self.storage_buffer_capacity(
            std::mem::size_of::<MatmulParams>() as u64,
            "arcweft-browser-prepared-matmul-params",
        );
        let layout = self.matmul_pipeline.get_bind_group_layout(0);
        let entries = bind_group_entries(&lhs, &rhs, &out, &params);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-browser-prepared-matmul-bind-group"),
            layout: &layout,
            entries: &entries,
        });
        self.stats.buffer_creations += 4;
        self.stats.bind_group_rebuilds += 1;
        Ok(BrowserPreparedMatmulF32 {
            capacity,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn prepare_matmul_add_f32(
        &mut self,
        capacity: BrowserMatmulCapacity,
    ) -> Result<BrowserPreparedMatmulAddF32, BrowserWebGpuError> {
        let output_len = checked_len_mul(capacity.rows, capacity.cols)?;
        let matmul = self.prepare_matmul_f32(capacity)?;
        let add = self.prepare_elementwise_f32(output_len)?;
        let add_layout = self.add_pipeline.get_bind_group_layout(0);
        let add_entries = bind_group_entries(&matmul.out, &add.rhs, &add.out, &add.params);
        let add_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-browser-prepared-matmul-add-bind-group"),
            layout: &add_layout,
            entries: &add_entries,
        });
        self.stats.bind_group_rebuilds += 1;
        Ok(BrowserPreparedMatmulAddF32 {
            matmul,
            add,
            add_bind_group,
        })
    }

    pub fn prepare_matmul_bias_add_f32(
        &mut self,
        capacity: BrowserMatmulCapacity,
    ) -> Result<BrowserPreparedMatmulBiasAddF32, BrowserWebGpuError> {
        let output_len = checked_len_mul(capacity.rows, capacity.cols)?;
        validate_f32_storage(self.limits, capacity.cols)?;
        validate_f32_storage(self.limits, output_len)?;
        let matmul = self.prepare_matmul_f32(capacity)?;
        let bias = self.storage_buffer_capacity(
            checked_f32_bytes(capacity.cols)?.max(std::mem::size_of::<f32>() as u64),
            "arcweft-browser-prepared-bias-add-bias",
        );
        let out = self.output_buffer_capacity(
            checked_f32_bytes(output_len)?.max(std::mem::size_of::<f32>() as u64),
            "arcweft-browser-prepared-bias-add-out",
        );
        let params = self.storage_buffer_capacity(
            std::mem::size_of::<BiasAddParams>() as u64,
            "arcweft-browser-prepared-bias-add-params",
        );
        let layout = self.bias_add_pipeline.get_bind_group_layout(0);
        let entries = bind_group_entries(&matmul.out, &bias, &out, &params);
        let bias_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-browser-prepared-matmul-bias-add-bind-group"),
            layout: &layout,
            entries: &entries,
        });
        self.stats.buffer_creations += 3;
        self.stats.bind_group_rebuilds += 1;
        Ok(BrowserPreparedMatmulBiasAddF32 {
            matmul,
            bias,
            out,
            params,
            bias_bind_group,
            output_capacity_len: output_len,
        })
    }

    pub fn prepare_resident_f32_graph(
        &mut self,
        spec: BrowserResidentF32GraphSpec,
    ) -> Result<BrowserPreparedResidentF32Graph, BrowserWebGpuError> {
        match spec {
            BrowserResidentF32GraphSpec::MatmulAdd { capacity } => self
                .prepare_matmul_add_f32(capacity)
                .map(BrowserPreparedResidentF32Graph::MatmulAdd),
            BrowserResidentF32GraphSpec::MatmulBiasAdd { capacity } => self
                .prepare_matmul_bias_add_f32(capacity)
                .map(BrowserPreparedResidentF32Graph::MatmulBiasAdd),
        }
    }

    pub async fn dispatch_prepared_elementwise_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        lhs: &[f32],
        rhs: &[f32],
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        self.upload_prepared_elementwise_f32(prepared, lhs, rhs)?;
        self.dispatch_resident_elementwise_f32(prepared, out).await
    }

    pub fn upload_prepared_elementwise_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        lhs: &[f32],
        rhs: &[f32],
    ) -> Result<(), BrowserWebGpuError> {
        if lhs.len() != rhs.len() {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "prepared elementwise add requires matching input lengths",
            ));
        }
        if lhs.len() > prepared.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "prepared elementwise add exceeded prepared capacity",
            ));
        }
        let params = AddParams {
            len: checked_u32(lhs.len())?,
            x_threads: checked_u32(add_groups(self.limits, lhs.len())?.0 as usize * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue
            .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
        self.queue
            .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        self.stats.bytes_uploaded += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
        self.stats.bytes_copied += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
        self.stats.buffer_reuse_hits += 3;
        Ok(())
    }

    pub fn upload_prepared_elementwise_rhs_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        rhs: &[f32],
    ) -> Result<(), BrowserWebGpuError> {
        if rhs.len() > prepared.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "prepared elementwise add rhs exceeded prepared capacity",
            ));
        }
        let params = AddParams {
            len: checked_u32(rhs.len())?,
            x_threads: checked_u32(add_groups(self.limits, rhs.len())?.0 as usize * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue
            .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        self.stats.bytes_uploaded += std::mem::size_of_val(rhs);
        self.stats.bytes_copied += std::mem::size_of_val(rhs);
        self.stats.buffer_reuse_hits += 2;
        Ok(())
    }

    pub async fn dispatch_resident_elementwise_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if out.len() > prepared.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident elementwise add exceeded prepared capacity",
            ));
        }
        if out.is_empty() {
            return Ok(());
        }
        let (groups_x, groups_y) = add_groups(self.limits, out.len())?;
        let pipeline = self.add_pipeline.clone();
        self.dispatch_prepared(
            &pipeline,
            &prepared.bind_group,
            &prepared.out,
            out,
            groups_x,
            groups_y,
        )
        .await?;
        self.record_prepared_readback(out);
        Ok(())
    }

    pub fn submit_resident_elementwise_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        len: usize,
    ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
        if len > prepared.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident elementwise add exceeded prepared capacity",
            ));
        }
        if len == 0 {
            return Ok(BrowserSubmittedF32 {
                readback: None,
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        let (groups_x, groups_y) = add_groups(self.limits, len)?;
        let pipeline = self.add_pipeline.clone();
        self.submit_prepared(
            &pipeline,
            &prepared.bind_group,
            &prepared.out,
            len,
            groups_x,
            groups_y,
        )
    }

    pub fn submit_resident_elementwise_f32_without_readback(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        len: usize,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        if len > prepared.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident elementwise add exceeded prepared capacity",
            ));
        }
        if len == 0 {
            return Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        let (groups_x, groups_y) = add_groups(self.limits, len)?;
        let pipeline = self.add_pipeline.clone();
        self.submit_prepared_without_readback(
            &pipeline,
            &prepared.bind_group,
            len,
            groups_x,
            groups_y,
        )
    }

    pub async fn read_resident_elementwise_f32(
        &mut self,
        prepared: &BrowserPreparedElementwiseF32,
        submission: BrowserResidentSubmission,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if out.len() != submission.len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident elementwise output length differs from readback target",
            ));
        }
        self.read_resident_f32(&prepared.out, submission, out).await
    }

    pub async fn dispatch_prepared_matmul_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        lhs: &[f32],
        rhs: &[f32],
        out: &mut [f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<(), BrowserWebGpuError> {
        self.upload_prepared_matmul_f32(prepared, lhs, rhs, rows, shared, cols)?;
        self.dispatch_resident_matmul_f32(prepared, out, rows, cols)
            .await
    }

    pub fn upload_prepared_matmul_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        lhs: &[f32],
        rhs: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<(), BrowserWebGpuError> {
        if rows > prepared.capacity.rows
            || shared > prepared.capacity.shared
            || cols > prepared.capacity.cols
        {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "prepared matmul exceeded prepared capacity",
            ));
        }
        if lhs.len() != checked_len_mul(rows, shared)?
            || rhs.len() != checked_len_mul(shared, cols)?
        {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "prepared matmul shape and buffer lengths differ",
            ));
        }
        let params = MatmulParams {
            rows: checked_u32(rows)?,
            shared: checked_u32(shared)?,
            cols: checked_u32(cols)?,
            _pad: 0,
        };
        self.queue
            .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
        self.queue
            .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        self.stats.bytes_uploaded += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
        self.stats.bytes_copied += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
        self.stats.buffer_reuse_hits += 3;
        Ok(())
    }

    pub fn upload_prepared_matmul_add_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulAddF32,
        lhs: &[f32],
        rhs: &[f32],
        add_rhs: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<(), BrowserWebGpuError> {
        let output_len = checked_len_mul(rows, cols)?;
        if add_rhs.len() != output_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "prepared matmul-add shape and add rhs length differ",
            ));
        }
        self.upload_prepared_matmul_f32(&prepared.matmul, lhs, rhs, rows, shared, cols)?;
        self.upload_prepared_elementwise_rhs_f32(&prepared.add, add_rhs)
    }

    pub fn upload_prepared_matmul_bias_add_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulBiasAddF32,
        lhs: &[f32],
        rhs: &[f32],
        bias: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<(), BrowserWebGpuError> {
        if bias.len() != cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "prepared matmul-bias-add shape and bias length differ",
            ));
        }
        self.upload_prepared_matmul_f32(&prepared.matmul, lhs, rhs, rows, shared, cols)?;
        let params = BiasAddParams {
            rows: checked_u32(rows)?,
            cols: checked_u32(cols)?,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue
            .write_buffer(&prepared.bias, 0, bytemuck::cast_slice(bias));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        self.stats.bytes_uploaded += std::mem::size_of_val(bias);
        self.stats.bytes_copied += std::mem::size_of_val(bias);
        self.stats.buffer_reuse_hits += 2;
        Ok(())
    }

    pub fn upload_prepared_resident_f32_graph(
        &mut self,
        prepared: &BrowserPreparedResidentF32Graph,
        inputs: BrowserResidentF32GraphInputs<'_>,
    ) -> Result<(), BrowserWebGpuError> {
        match (prepared, inputs) {
            (
                BrowserPreparedResidentF32Graph::MatmulAdd(prepared),
                BrowserResidentF32GraphInputs::MatmulAdd(inputs),
            ) => self.upload_prepared_matmul_add_f32(
                prepared,
                inputs.lhs,
                inputs.rhs,
                inputs.add_rhs,
                inputs.shape.rows,
                inputs.shape.shared,
                inputs.shape.cols,
            ),
            (
                BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared),
                BrowserResidentF32GraphInputs::MatmulBiasAdd(inputs),
            ) => self.upload_prepared_matmul_bias_add_f32(
                prepared,
                inputs.lhs,
                inputs.rhs,
                inputs.bias,
                inputs.shape.rows,
                inputs.shape.shared,
                inputs.shape.cols,
            ),
            (
                BrowserPreparedResidentF32Graph::MatmulAdd(_),
                BrowserResidentF32GraphInputs::MatmulBiasAdd(_),
            )
            | (
                BrowserPreparedResidentF32Graph::MatmulBiasAdd(_),
                BrowserResidentF32GraphInputs::MatmulAdd(_),
            ) => Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "prepared resident graph inputs do not match the prepared graph fragment",
            )),
        }
    }

    pub async fn dispatch_resident_matmul_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        out: &mut [f32],
        rows: usize,
        cols: usize,
    ) -> Result<(), BrowserWebGpuError> {
        if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul exceeded prepared capacity",
            ));
        }
        if out.len() != checked_len_mul(rows, cols)? {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident matmul output length differs from shape",
            ));
        }
        if out.is_empty() {
            return Ok(());
        }
        let pipeline = self.matmul_pipeline.clone();
        self.dispatch_prepared(
            &pipeline,
            &prepared.bind_group,
            &prepared.out,
            out,
            checked_workgroups(self.limits, cols.div_ceil(16))?,
            checked_workgroups(self.limits, rows.div_ceil(16))?,
        )
        .await?;
        self.record_prepared_readback(out);
        Ok(())
    }

    pub fn submit_resident_matmul_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        rows: usize,
        cols: usize,
    ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
        if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul exceeded prepared capacity",
            ));
        }
        let len = checked_len_mul(rows, cols)?;
        if len == 0 {
            return Ok(BrowserSubmittedF32 {
                readback: None,
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        let pipeline = self.matmul_pipeline.clone();
        self.submit_prepared(
            &pipeline,
            &prepared.bind_group,
            &prepared.out,
            len,
            checked_workgroups(self.limits, cols.div_ceil(16))?,
            checked_workgroups(self.limits, rows.div_ceil(16))?,
        )
    }

    pub fn submit_resident_matmul_f32_without_readback(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        rows: usize,
        cols: usize,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul exceeded prepared capacity",
            ));
        }
        let len = checked_len_mul(rows, cols)?;
        if len == 0 {
            return Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        let pipeline = self.matmul_pipeline.clone();
        self.submit_prepared_without_readback(
            &pipeline,
            &prepared.bind_group,
            len,
            checked_workgroups(self.limits, cols.div_ceil(16))?,
            checked_workgroups(self.limits, rows.div_ceil(16))?,
        )
    }

    pub fn submit_resident_matmul_add_f32_without_readback(
        &mut self,
        prepared: &BrowserPreparedMatmulAddF32,
        rows: usize,
        cols: usize,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-add exceeded prepared matmul capacity",
            ));
        }
        let len = checked_len_mul(rows, cols)?;
        if len > prepared.add.capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-add exceeded prepared add capacity",
            ));
        }
        if len == 0 {
            return Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        self.ensure_healthy()?;
        validate_f32_storage(self.limits, len)?;
        let matmul_pipeline = self.matmul_pipeline.clone();
        let add_pipeline = self.add_pipeline.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-resident-matmul-add-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-resident-matmul-add-matmul-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&matmul_pipeline);
            pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
                1,
            );
        }
        {
            let (groups_x, groups_y) = add_groups(self.limits, len)?;
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-resident-matmul-add-add-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&add_pipeline);
            pass.set_bind_group(0, &prepared.add_bind_group, &[]);
            pass.dispatch_workgroups(groups_x.max(1), groups_y.max(1), 1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 2;
        self.stats.async_submissions += 1;
        self.stats.pipeline_cache_hits += 2;
        Ok(BrowserResidentSubmission {
            len,
            submitted_at_ms: browser_now_ms(),
        })
    }

    pub fn submit_resident_matmul_bias_add_f32_without_readback(
        &mut self,
        prepared: &BrowserPreparedMatmulBiasAddF32,
        rows: usize,
        cols: usize,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-bias-add exceeded prepared matmul capacity",
            ));
        }
        let len = checked_len_mul(rows, cols)?;
        if len > prepared.output_capacity_len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-bias-add exceeded prepared output capacity",
            ));
        }
        if len == 0 {
            return Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            });
        }
        self.ensure_healthy()?;
        validate_f32_storage(self.limits, len)?;
        let matmul_pipeline = self.matmul_pipeline.clone();
        let bias_add_pipeline = self.bias_add_pipeline.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-resident-matmul-bias-add-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-resident-matmul-bias-add-matmul-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&matmul_pipeline);
            pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
                1,
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-resident-matmul-bias-add-bias-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&bias_add_pipeline);
            pass.set_bind_group(0, &prepared.bias_bind_group, &[]);
            pass.dispatch_workgroups(
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
                1,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 2;
        self.stats.async_submissions += 1;
        self.stats.pipeline_cache_hits += 2;
        Ok(BrowserResidentSubmission {
            len,
            submitted_at_ms: browser_now_ms(),
        })
    }

    pub fn submit_prepared_resident_f32_graph_without_readback(
        &mut self,
        prepared: &BrowserPreparedResidentF32Graph,
        shape: BrowserMatmulAddF32Shape,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        match prepared {
            BrowserPreparedResidentF32Graph::MatmulAdd(prepared) => self
                .submit_resident_matmul_add_f32_without_readback(prepared, shape.rows, shape.cols),
            BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared) => self
                .submit_resident_matmul_bias_add_f32_without_readback(
                    prepared, shape.rows, shape.cols,
                ),
        }
    }

    pub async fn read_resident_matmul_add_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulAddF32,
        submission: BrowserResidentSubmission,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-add exceeded prepared capacity",
            ));
        }
        if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident matmul-add output length differs from readback target",
            ));
        }
        self.read_resident_f32(&prepared.add.out, submission, out)
            .await
    }

    pub async fn read_resident_matmul_bias_add_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulBiasAddF32,
        submission: BrowserResidentSubmission,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul-bias-add exceeded prepared capacity",
            ));
        }
        if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident matmul-bias-add output length differs from readback target",
            ));
        }
        self.read_resident_f32(&prepared.out, submission, out).await
    }

    pub async fn read_prepared_resident_f32_graph(
        &mut self,
        prepared: &BrowserPreparedResidentF32Graph,
        submission: BrowserResidentSubmission,
        shape: BrowserMatmulAddF32Shape,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        match prepared {
            BrowserPreparedResidentF32Graph::MatmulAdd(prepared) => {
                self.read_resident_matmul_add_f32(prepared, submission, shape.rows, shape.cols, out)
                    .await
            }
            BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared) => {
                self.read_resident_matmul_bias_add_f32(
                    prepared, submission, shape.rows, shape.cols, out,
                )
                .await
            }
        }
    }

    pub async fn read_resident_matmul_f32(
        &mut self,
        prepared: &BrowserPreparedMatmulF32,
        submission: BrowserResidentSubmission,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "resident matmul exceeded prepared capacity",
            ));
        }
        if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident matmul output length differs from readback target",
            ));
        }
        self.read_resident_f32(&prepared.out, submission, out).await
    }

    pub async fn read_submitted_f32(
        &mut self,
        submitted: BrowserSubmittedF32,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if out.len() != submitted.len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "submitted browser GPU output length differs from readback target",
            ));
        }
        let Some(buffer) = submitted.readback else {
            return Ok(());
        };
        self.read_owned_staging_buffer(buffer, submitted.submitted_at_ms, out)
            .await?;
        self.in_flight = self.in_flight.saturating_sub(1);
        self.stats.async_readbacks += 1;
        Ok(())
    }

    async fn dispatch_add(
        &mut self,
        lhs: &[f32],
        rhs: &[f32],
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        validate_f32_storage(self.limits, lhs.len())?;
        validate_f32_storage(self.limits, rhs.len())?;
        validate_f32_storage(self.limits, out.len())?;
        let (groups_x, groups_y) = add_groups(self.limits, out.len())?;
        let params = AddParams {
            len: checked_u32(out.len())?,
            x_threads: checked_u32(groups_x as usize * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        let pipeline = self.add_pipeline.clone();
        self.dispatch(
            &pipeline,
            &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
            bytemuck::bytes_of(&params),
            out,
            groups_x,
            groups_y,
        )
        .await
    }

    async fn dispatch(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        input_bytes: &[&[u8]],
        params_bytes: &[u8],
        out: &mut [f32],
        workgroups_x: u32,
        workgroups_y: u32,
    ) -> Result<(), BrowserWebGpuError> {
        if out.is_empty() {
            return Ok(());
        }
        self.ensure_healthy()?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let lhs = storage_buffer(&self.device, input_bytes[0], "arcweft-browser-math-lhs");
        let rhs = storage_buffer(&self.device, input_bytes[1], "arcweft-browser-math-rhs");
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-browser-math-out"),
            size: checked_f32_bytes(out.len())?,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = storage_buffer(&self.device, params_bytes, "arcweft-browser-math-params");
        let layout = pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-browser-math-bind-group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: lhs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: rhs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: out_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-math-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-math-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
        }
        self.encode_readback(&mut encoder, &out_buffer, out.len())?;
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 1;
        self.stats.bytes_uploaded += input_bytes.iter().map(|bytes| bytes.len()).sum::<usize>();
        self.stats.bytes_downloaded += std::mem::size_of_val(out);
        self.stats.bytes_copied +=
            input_bytes.iter().map(|bytes| bytes.len()).sum::<usize>() + std::mem::size_of_val(out);
        self.stats.buffer_creations += 4;
        if let Some(error) = scope.pop().await {
            return Err(map_wgpu_error(error));
        }
        self.read_staging_buffer(out).await
    }

    async fn dispatch_prepared(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        source: &wgpu::Buffer,
        out: &mut [f32],
        workgroups_x: u32,
        workgroups_y: u32,
    ) -> Result<(), BrowserWebGpuError> {
        self.ensure_healthy()?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-prepared-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-prepared-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
        }
        self.encode_readback(&mut encoder, source, out.len())?;
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 1;
        if let Some(error) = scope.pop().await {
            return Err(map_wgpu_error(error));
        }
        self.read_staging_buffer(out).await
    }

    fn submit_prepared(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        source: &wgpu::Buffer,
        elements: usize,
        workgroups_x: u32,
        workgroups_y: u32,
    ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
        self.ensure_healthy()?;
        let byte_len = checked_f32_bytes(elements)?;
        let readback = self.take_async_readback_buffer(byte_len)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-async-prepared-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-async-prepared-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
        }
        encoder.copy_buffer_to_buffer(source, 0, &readback.buffer, 0, byte_len);
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 1;
        self.stats.async_submissions += 1;
        self.stats.bytes_downloaded += std::mem::size_of::<f32>() * elements;
        self.stats.bytes_copied += std::mem::size_of::<f32>() * elements;
        self.stats.buffer_reuse_hits += 1;
        self.stats.pipeline_cache_hits += 1;
        self.in_flight += 1;
        self.stats.max_in_flight = self.stats.max_in_flight.max(self.in_flight);
        Ok(BrowserSubmittedF32 {
            readback: Some(readback),
            len: elements,
            submitted_at_ms: browser_now_ms(),
        })
    }

    fn submit_prepared_without_readback(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        elements: usize,
        workgroups_x: u32,
        workgroups_y: u32,
    ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
        self.ensure_healthy()?;
        validate_f32_storage(self.limits, elements)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-resident-prepared-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-browser-resident-prepared-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.stats.dispatches += 1;
        self.stats.async_submissions += 1;
        self.stats.pipeline_cache_hits += 1;
        Ok(BrowserResidentSubmission {
            len: elements,
            submitted_at_ms: browser_now_ms(),
        })
    }

    async fn read_resident_f32(
        &mut self,
        source: &wgpu::Buffer,
        submission: BrowserResidentSubmission,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        if out.len() != submission.len {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                "resident output length differs from readback target",
            ));
        }
        if out.is_empty() {
            return Ok(());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-browser-resident-readback-encoder"),
            });
        self.encode_readback(&mut encoder, source, out.len())?;
        self.queue.submit(Some(encoder.finish()));
        self.stats.bytes_downloaded += std::mem::size_of_val(out);
        self.stats.bytes_copied += std::mem::size_of_val(out);
        self.stats.buffer_reuse_hits += 1;
        self.read_staging_buffer_from(submission.submitted_at_ms, out)
            .await
    }

    fn encode_readback(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        elements: usize,
    ) -> Result<(), BrowserWebGpuError> {
        let byte_len = checked_f32_bytes(elements)?;
        let readback = self.ensure_readback_buffer(byte_len)?;
        encoder.copy_buffer_to_buffer(source, 0, readback, 0, byte_len);
        Ok(())
    }

    fn ensure_readback_buffer(
        &mut self,
        byte_len: u64,
    ) -> Result<&wgpu::Buffer, BrowserWebGpuError> {
        validate_byte_len(self.limits, byte_len)?;
        let byte_len_usize = checked_usize_bytes(byte_len)?;
        let needs_new = self
            .readback
            .as_ref()
            .is_none_or(|buffer| buffer.byte_len < byte_len_usize);
        if needs_new {
            self.readback = Some(ReusableReadbackBuffer {
                buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("arcweft-browser-math-readback"),
                    size: byte_len.max(std::mem::size_of::<f32>() as u64),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                byte_len: byte_len_usize,
            });
            self.stats.readback_buffer_creations += 1;
        } else {
            self.stats.readback_buffer_reuse_hits += 1;
        }
        Ok(&self
            .readback
            .as_ref()
            .expect("browser readback buffer was initialized")
            .buffer)
    }

    fn take_async_readback_buffer(
        &mut self,
        byte_len: u64,
    ) -> Result<ReusableReadbackBuffer, BrowserWebGpuError> {
        validate_byte_len(self.limits, byte_len)?;
        let byte_len_usize = checked_usize_bytes(byte_len)?;
        if let Some(buffer) = self.async_readback.take()
            && buffer.byte_len >= byte_len_usize
        {
            self.stats.readback_buffer_reuse_hits += 1;
            return Ok(buffer);
        }
        self.stats.readback_buffer_creations += 1;
        Ok(ReusableReadbackBuffer {
            buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-browser-async-readback"),
                size: byte_len.max(std::mem::size_of::<f32>() as u64),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            byte_len: byte_len_usize,
        })
    }

    async fn read_staging_buffer(&mut self, out: &mut [f32]) -> Result<(), BrowserWebGpuError> {
        self.read_staging_buffer_from(browser_now_ms(), out).await
    }

    async fn read_staging_buffer_from(
        &mut self,
        submitted_at_ms: f64,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        let byte_len = std::mem::size_of_val(out);
        let buffer = &self
            .readback
            .as_ref()
            .expect("browser readback buffer was initialized")
            .buffer;
        let (sender, receiver) = oneshot::channel();
        buffer
            .slice(0..byte_len as u64)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        receiver
            .await
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::MapFailed,
                    error.to_string(),
                )
            })?
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::MapFailed,
                    error.to_string(),
                )
            })?;
        self.stats.map_count += 1;
        self.stats.map_wait_ms += browser_now_ms() - submitted_at_ms;
        {
            let mapped = buffer.slice(0..byte_len as u64).get_mapped_range();
            let values: &[f32] = bytemuck::cast_slice(&mapped);
            out.copy_from_slice(values);
        }
        buffer.unmap();
        Ok(())
    }

    async fn read_owned_staging_buffer(
        &mut self,
        buffer: ReusableReadbackBuffer,
        submitted_at_ms: f64,
        out: &mut [f32],
    ) -> Result<(), BrowserWebGpuError> {
        let byte_len = std::mem::size_of_val(out);
        let (sender, receiver) = oneshot::channel();
        buffer
            .buffer
            .slice(0..byte_len as u64)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        receiver
            .await
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::MapFailed,
                    error.to_string(),
                )
            })?
            .map_err(|error| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::MapFailed,
                    error.to_string(),
                )
            })?;
        self.stats.map_count += 1;
        self.stats.map_wait_ms += browser_now_ms() - submitted_at_ms;
        {
            let mapped = buffer.buffer.slice(0..byte_len as u64).get_mapped_range();
            let values: &[f32] = bytemuck::cast_slice(&mapped);
            out.copy_from_slice(values);
        }
        buffer.buffer.unmap();
        self.async_readback = Some(buffer);
        Ok(())
    }

    fn storage_buffer_capacity(&self, byte_len: u64, label: &'static str) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn output_buffer_capacity(&self, byte_len: u64, label: &'static str) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn record_prepared_readback(&mut self, out: &[f32]) {
        let downloaded = std::mem::size_of_val(out);
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += downloaded;
        self.stats.buffer_reuse_hits += 1;
        self.stats.pipeline_cache_hits += 1;
    }

    fn ensure_healthy(&self) -> Result<(), BrowserWebGpuError> {
        if let Ok(slot) = self.device_lost.lock()
            && let Some(message) = slot.as_ref()
        {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::DeviceLost,
                message.clone(),
            ));
        }
        if let Ok(slot) = self.uncaptured_error.lock()
            && let Some(message) = slot.as_ref()
        {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::ValidationError,
                message.clone(),
            ));
        }
        Ok(())
    }
}

fn storage_buffer(device: &wgpu::Device, contents: &[u8], label: &'static str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

fn bind_group_entries<'a>(
    lhs: &'a wgpu::Buffer,
    rhs: &'a wgpu::Buffer,
    out: &'a wgpu::Buffer,
    params: &'a wgpu::Buffer,
) -> [wgpu::BindGroupEntry<'a>; 4] {
    [
        wgpu::BindGroupEntry {
            binding: 0,
            resource: lhs.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: rhs.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: out.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 3,
            resource: params.as_entire_binding(),
        },
    ]
}

const fn matmul_capacity_covers(
    available: BrowserMatmulCapacity,
    required: BrowserMatmulCapacity,
) -> bool {
    available.rows >= required.rows
        && available.shared >= required.shared
        && available.cols >= required.cols
}

impl From<wgpu::Limits> for BrowserWebGpuLimits {
    fn from(limits: wgpu::Limits) -> Self {
        Self {
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_buffer_size: limits.max_buffer_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        }
    }
}

fn checked_u32(value: usize) -> Result<u32, BrowserWebGpuError> {
    u32::try_from(value).map_err(|_| {
        BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::WorkgroupCountTooLarge,
            format!("browser GPU dispatch dimension {value} exceeds u32"),
        )
    })
}

fn checked_len_mul(lhs: usize, rhs: usize) -> Result<usize, BrowserWebGpuError> {
    lhs.checked_mul(rhs).ok_or_else(|| {
        BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::BufferSizeTooLarge,
            "matrix/tensor shape multiplication overflowed",
        )
    })
}

fn checked_f32_bytes(len: usize) -> Result<u64, BrowserWebGpuError> {
    len.checked_mul(std::mem::size_of::<f32>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| {
            BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::BufferSizeTooLarge,
                "f32 buffer byte size overflowed",
            )
        })
}

fn checked_usize_bytes(byte_len: u64) -> Result<usize, BrowserWebGpuError> {
    usize::try_from(byte_len).map_err(|_| {
        BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::BufferSizeTooLarge,
            "browser WebGPU buffer byte size exceeds usize",
        )
    })
}

fn validate_f32_storage(limits: BrowserWebGpuLimits, len: usize) -> Result<(), BrowserWebGpuError> {
    validate_byte_len(limits, checked_f32_bytes(len)?)
}

fn validate_byte_len(limits: BrowserWebGpuLimits, byte_len: u64) -> Result<(), BrowserWebGpuError> {
    if byte_len > limits.max_buffer_size {
        return Err(BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::BufferSizeTooLarge,
            "browser WebGPU buffer exceeds max_buffer_size",
        ));
    }
    if byte_len > limits.max_storage_buffer_binding_size {
        return Err(BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::StorageBufferTooLarge,
            "browser WebGPU storage buffer exceeds max_storage_buffer_binding_size",
        ));
    }
    Ok(())
}

fn checked_workgroups(
    limits: BrowserWebGpuLimits,
    groups: usize,
) -> Result<u32, BrowserWebGpuError> {
    let groups = checked_u32(groups)?;
    if groups > limits.max_compute_workgroups_per_dimension {
        return Err(BrowserWebGpuError::fallback(
            BrowserWebGpuFallbackReason::WorkgroupCountTooLarge,
            "browser WebGPU dispatch exceeds max_compute_workgroups_per_dimension",
        ));
    }
    Ok(groups)
}

fn add_groups(limits: BrowserWebGpuLimits, len: usize) -> Result<(u32, u32), BrowserWebGpuError> {
    let groups = len.div_ceil(256).max(1);
    let max_per_dimension = usize::try_from(limits.max_compute_workgroups_per_dimension)
        .unwrap_or(usize::MAX)
        .max(1);
    let groups_x = groups.min(max_per_dimension);
    let groups_y = groups.div_ceil(groups_x).max(1);
    Ok((
        checked_workgroups(limits, groups_x)?,
        checked_workgroups(limits, groups_y)?,
    ))
}

fn map_wgpu_error(error: wgpu::Error) -> BrowserWebGpuError {
    let reason = match error {
        wgpu::Error::Validation { .. } => BrowserWebGpuFallbackReason::ValidationError,
        wgpu::Error::OutOfMemory { .. } => BrowserWebGpuFallbackReason::OutOfMemory,
        wgpu::Error::Internal { .. } => BrowserWebGpuFallbackReason::InternalError,
    };
    BrowserWebGpuError::fallback(reason, error.to_string())
}

fn bool_property(value: &wasm_bindgen::JsValue, name: &str) -> bool {
    js_sys::Reflect::get(value, &name.into())
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn browser_now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now())
}

const MATMUL_SHADER: &str = r"
struct MatrixParams {
    rows: u32,
    k_len: u32,
    cols: u32,
    pad: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: MatrixParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    var acc = 0.0;
    for (var k = 0u; k < params.k_len; k = k + 1u) {
        acc = acc + lhs[row * params.k_len + k] * rhs[k * params.cols + col];
    }
    out[row * params.cols + col] = acc;
}
";

const ADD_SHADER: &str = r"
struct AddParams {
    len: u32,
    x_threads: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: AddParams;

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.y * params.x_threads + id.x;
    if (index >= params.len) {
        return;
    }
    out[index] = lhs[index] + rhs[index];
}
";

const BIAS_ADD_SHADER: &str = r"
struct BiasAddParams {
    rows: u32,
    cols: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> bias: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: BiasAddParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    let index = row * params.cols + col;
    out[index] = lhs[index] + bias[col];
}
";
