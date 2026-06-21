use super::{DenseMatrixF32, DenseTensorF32, RuntimeMathAcceleratorError, RuntimeMathError};
use bytemuck::{Pod, Zeroable};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

pub struct WgpuMathContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    matmul_pipeline: wgpu::ComputePipeline,
    add_pipeline: wgpu::ComputePipeline,
    bias_add_pipeline: wgpu::ComputePipeline,
    readback: Option<ReusableReadbackBuffer>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GpuReadbackUsage {
    pub created: bool,
    pub reused: bool,
}

struct ReusableReadbackBuffer {
    buffer: wgpu::Buffer,
    byte_len: usize,
}

pub struct PreparedAddBuffers {
    len: usize,
    lhs: wgpu::Buffer,
    rhs: wgpu::Buffer,
    out: wgpu::Buffer,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PreparedAddBuffers {
    pub const fn len(&self) -> usize {
        self.len
    }
}

pub struct PreparedMatmulBuffers {
    rows: usize,
    shared: usize,
    cols: usize,
    lhs: wgpu::Buffer,
    rhs: wgpu::Buffer,
    out: wgpu::Buffer,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PreparedMatmulBuffers {
    pub const fn len(&self) -> usize {
        self.rows * self.cols
    }
}

pub struct PreparedMatmulBiasAddBuffers {
    matmul: PreparedMatmulBuffers,
    bias: wgpu::Buffer,
    out: wgpu::Buffer,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PreparedMatmulBiasAddBuffers {
    pub const fn len(&self) -> usize {
        self.matmul.rows * self.matmul.cols
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MatmulParams {
    rows: u32,
    shared: u32,
    cols: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AddParams {
    len: u32,
    x_threads: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BiasAddParams {
    rows: u32,
    cols: u32,
    _pad1: u32,
    _pad2: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MatmulShape {
    pub(super) rows: usize,
    pub(super) shared: usize,
    pub(super) cols: usize,
}

impl WgpuMathContext {
    pub fn new() -> Result<Self, RuntimeMathAcceleratorError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("arcweft-runtime-math"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
        let matmul_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-matmul-f32"),
            source: wgpu::ShaderSource::Wgsl(MATMUL_SHADER.into()),
        });
        let add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-add-f32"),
            source: wgpu::ShaderSource::Wgsl(ADD_SHADER.into()),
        });
        let bias_add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-bias-add-f32"),
            source: wgpu::ShaderSource::Wgsl(BIAS_ADD_SHADER.into()),
        });
        let matmul_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-matmul-f32"),
            layout: None,
            module: &matmul_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-add-f32"),
            layout: None,
            module: &add_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let bias_add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arcweft-bias-add-f32"),
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
        })
    }

    pub fn matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
        if lhs.cols() != rhs.rows() {
            let out = lhs.matmul_scalar(rhs)?;
            return Ok((out, GpuReadbackUsage::default()));
        }
        let out_len = lhs.rows() * rhs.cols();
        let mut out = vec![0.0; out_len];
        let readback = self.dispatch_matmul(
            lhs.values(),
            rhs.values(),
            &mut out,
            lhs.rows(),
            lhs.cols(),
            rhs.cols(),
        )?;
        Ok((DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?, readback))
    }

    pub fn matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            let out = lhs.add_scalar(rhs)?;
            return Ok((out, GpuReadbackUsage::default()));
        }
        let mut out = vec![0.0; lhs.values().len()];
        let readback = self.dispatch_add(lhs.values(), rhs.values(), &mut out)?;
        Ok((DenseMatrixF32::new(lhs.rows(), lhs.cols(), out)?, readback))
    }

    pub fn tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<(DenseTensorF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            let out = lhs.add_scalar(rhs)?;
            return Ok((out, GpuReadbackUsage::default()));
        }
        let mut out = vec![0.0; lhs.values().len()];
        let readback = self.dispatch_add(lhs.values(), rhs.values(), &mut out)?;
        Ok((
            DenseTensorF32::new(lhs.shape().dims().to_vec(), out)?,
            readback,
        ))
    }

    pub fn matmul_bias_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul_bias_add",
            }
            .into());
        }
        if bias.shape().dims() != [rhs.cols()] {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "matmul_bias_add expected bias shape [{}], found {:?}",
                rhs.cols(),
                bias.shape().dims()
            )));
        }
        let prepared = self.prepare_matmul_bias_add(
            lhs.values(),
            rhs.values(),
            bias.values(),
            lhs.rows(),
            lhs.cols(),
            rhs.cols(),
        )?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        let readback = self.dispatch_prepared_matmul_bias_add(&prepared, &mut out)?;
        Ok((DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?, readback))
    }

    pub fn prepare_add(
        &self,
        lhs: &[f32],
        rhs: &[f32],
    ) -> Result<PreparedAddBuffers, RuntimeMathAcceleratorError> {
        if lhs.len() != rhs.len() {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared add expected matching input lengths, got {} and {}",
                lhs.len(),
                rhs.len()
            )));
        }
        let len = lhs.len();
        let byte_len = std::mem::size_of_val(lhs).max(std::mem::size_of::<f32>());
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-prepared-out"),
            size: byte_len as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = AddParams {
            len: checked_u32(len)?,
            x_threads: checked_u32(add_groups(len).0 * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        let lhs = storage_buffer(
            &self.device,
            bytemuck::cast_slice(lhs),
            "arcweft-math-prepared-lhs",
        );
        let rhs = storage_buffer(
            &self.device,
            bytemuck::cast_slice(rhs),
            "arcweft-math-prepared-rhs",
        );
        let params = storage_buffer(
            &self.device,
            bytemuck::bytes_of(&params),
            "arcweft-math-prepared-params",
        );
        let layout = self.add_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-prepared-bind-group"),
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
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        Ok(PreparedAddBuffers {
            len,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn prepare_add_capacity(
        &self,
        capacity_len: usize,
    ) -> Result<PreparedAddBuffers, RuntimeMathAcceleratorError> {
        let _ = checked_u32(capacity_len)?;
        let byte_len = capacity_len
            .saturating_mul(std::mem::size_of::<f32>())
            .max(std::mem::size_of::<f32>());
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-prepared-out"),
            size: byte_len as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let lhs = storage_buffer_capacity(&self.device, byte_len, "arcweft-math-prepared-lhs");
        let rhs = storage_buffer_capacity(&self.device, byte_len, "arcweft-math-prepared-rhs");
        let params = storage_buffer_capacity(
            &self.device,
            std::mem::size_of::<AddParams>(),
            "arcweft-math-prepared-params",
        );
        let layout = self.add_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-prepared-bind-group"),
            layout: &layout,
            entries: &bind_group_entries(&lhs, &rhs, &out, &params),
        });
        Ok(PreparedAddBuffers {
            len: capacity_len,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn update_prepared_add(
        &self,
        prepared: &PreparedAddBuffers,
        lhs: &[f32],
        rhs: &[f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if lhs.len() != rhs.len() || lhs.len() > prepared.len {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared add capacity is {} value(s), got {} and {}",
                prepared.len,
                lhs.len(),
                rhs.len()
            )));
        }
        let params = AddParams {
            len: checked_u32(lhs.len())?,
            x_threads: checked_u32(add_groups(lhs.len()).0 * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue
            .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
        self.queue
            .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        Ok(())
    }

    pub fn prepare_matmul(
        &self,
        lhs: &[f32],
        rhs: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<PreparedMatmulBuffers, RuntimeMathAcceleratorError> {
        if lhs.len() != rows * shared || rhs.len() != shared * cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul expected lhs {}x{} and rhs {}x{}, got {} and {} value(s)",
                rows,
                shared,
                shared,
                cols,
                lhs.len(),
                rhs.len()
            )));
        }
        let byte_len = rows
            .saturating_mul(cols)
            .saturating_mul(std::mem::size_of::<f32>())
            .max(std::mem::size_of::<f32>());
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-prepared-matmul-out"),
            size: byte_len as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = MatmulParams {
            rows: checked_u32(rows)?,
            shared: checked_u32(shared)?,
            cols: checked_u32(cols)?,
            _pad: 0,
        };
        let lhs = storage_buffer(
            &self.device,
            bytemuck::cast_slice(lhs),
            "arcweft-math-prepared-matmul-lhs",
        );
        let rhs = storage_buffer(
            &self.device,
            bytemuck::cast_slice(rhs),
            "arcweft-math-prepared-matmul-rhs",
        );
        let params = storage_buffer(
            &self.device,
            bytemuck::bytes_of(&params),
            "arcweft-math-prepared-matmul-params",
        );
        let layout = self.matmul_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-prepared-matmul-bind-group"),
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
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        Ok(PreparedMatmulBuffers {
            rows,
            shared,
            cols,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn prepare_matmul_capacity(
        &self,
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<PreparedMatmulBuffers, RuntimeMathAcceleratorError> {
        let _ = checked_u32(rows)?;
        let _ = checked_u32(shared)?;
        let _ = checked_u32(cols)?;
        let lhs_len = rows.saturating_mul(shared);
        let rhs_len = shared.saturating_mul(cols);
        let out_len = rows.saturating_mul(cols);
        let lhs = storage_buffer_capacity(
            &self.device,
            lhs_len
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>()),
            "arcweft-math-prepared-matmul-lhs",
        );
        let rhs = storage_buffer_capacity(
            &self.device,
            rhs_len
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>()),
            "arcweft-math-prepared-matmul-rhs",
        );
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-prepared-matmul-out"),
            size: out_len
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = storage_buffer_capacity(
            &self.device,
            std::mem::size_of::<MatmulParams>(),
            "arcweft-math-prepared-matmul-params",
        );
        let layout = self.matmul_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-prepared-matmul-bind-group"),
            layout: &layout,
            entries: &bind_group_entries(&lhs, &rhs, &out, &params),
        });
        Ok(PreparedMatmulBuffers {
            rows,
            shared,
            cols,
            lhs,
            rhs,
            out,
            params,
            bind_group,
        })
    }

    pub fn update_prepared_matmul(
        &self,
        prepared: &PreparedMatmulBuffers,
        lhs: &[f32],
        rhs: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || shared > prepared.shared || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul capacity is {}x{} and {}x{}, got {}x{} and {}x{}",
                prepared.rows,
                prepared.shared,
                prepared.shared,
                prepared.cols,
                rows,
                shared,
                shared,
                cols
            )));
        }
        if lhs.len() != rows * shared || rhs.len() != shared * cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul expected lhs {}x{} and rhs {}x{}, got {} and {} value(s)",
                rows,
                shared,
                shared,
                cols,
                lhs.len(),
                rhs.len()
            )));
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
        Ok(())
    }

    pub fn prepare_matmul_bias_add(
        &self,
        lhs: &[f32],
        rhs: &[f32],
        bias: &[f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
        if bias.len() != cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add expected bias length {cols}, got {}",
                bias.len()
            )));
        }
        let matmul = self.prepare_matmul(lhs, rhs, rows, shared, cols)?;
        let bias = storage_buffer(
            &self.device,
            bytemuck::cast_slice(bias),
            "arcweft-math-prepared-matmul-bias-add-bias",
        );
        self.prepare_matmul_bias_add_output(matmul, bias, rows, cols)
    }

    pub fn prepare_matmul_bias_add_capacity(
        &self,
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
        let matmul = self.prepare_matmul_capacity(rows, shared, cols)?;
        let bias = storage_buffer_capacity(
            &self.device,
            cols.saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>()),
            "arcweft-math-prepared-matmul-bias-add-bias",
        );
        self.prepare_matmul_bias_add_output(matmul, bias, rows, cols)
    }

    fn prepare_matmul_bias_add_output(
        &self,
        matmul: PreparedMatmulBuffers,
        bias: wgpu::Buffer,
        rows: usize,
        cols: usize,
    ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
        let out_len = rows.saturating_mul(cols);
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-prepared-matmul-bias-add-out"),
            size: out_len
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = BiasAddParams {
            rows: checked_u32(rows)?,
            cols: checked_u32(cols)?,
            _pad1: 0,
            _pad2: 0,
        };
        let params = storage_buffer(
            &self.device,
            bytemuck::bytes_of(&params),
            "arcweft-math-prepared-matmul-bias-add-params",
        );
        let layout = self.bias_add_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-prepared-matmul-bias-add-bind-group"),
            layout: &layout,
            entries: &bind_group_entries(&matmul.out, &bias, &out, &params),
        });
        Ok(PreparedMatmulBiasAddBuffers {
            matmul,
            bias,
            out,
            params,
            bind_group,
        })
    }

    pub fn update_prepared_matmul_bias_add(
        &self,
        prepared: &PreparedMatmulBiasAddBuffers,
        lhs: &[f32],
        rhs: &[f32],
        bias: &[f32],
        shape: MatmulShape,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if bias.len() != shape.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add expected bias length {}, got {}",
                shape.cols,
                bias.len()
            )));
        }
        self.update_prepared_matmul(
            &prepared.matmul,
            lhs,
            rhs,
            shape.rows,
            shape.shared,
            shape.cols,
        )?;
        let params = BiasAddParams {
            rows: checked_u32(shape.rows)?,
            cols: checked_u32(shape.cols)?,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue
            .write_buffer(&prepared.bias, 0, bytemuck::cast_slice(bias));
        self.queue
            .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
        Ok(())
    }

    pub fn dispatch_prepared_add(
        &mut self,
        prepared: &PreparedAddBuffers,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if out.len() > prepared.len {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared add capacity is {} value(s), got {}",
                prepared.len,
                out.len()
            )));
        }
        if out.is_empty() {
            return Ok(GpuReadbackUsage::default());
        }
        let (groups_x, groups_y) = add_groups(out.len());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.add_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(checked_u32(groups_x)?, checked_u32(groups_y)?, 1);
        }
        let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
        self.queue.submit(Some(encoder.finish()));
        self.read_staging_buffer(out)?;
        Ok(readback)
    }

    pub fn submit_prepared_add_without_readback(
        &self,
        prepared: &PreparedAddBuffers,
        len: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if len > prepared.len {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared add capacity is {} value(s), got {}",
                prepared.len, len
            )));
        }
        if len == 0 {
            return Ok(());
        }
        let (groups_x, groups_y) = add_groups(len);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-add-submit-only-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-add-submit-only-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.add_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(checked_u32(groups_x)?, checked_u32(groups_y)?, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    pub fn read_prepared_add_output(
        &mut self,
        prepared: &PreparedAddBuffers,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if out.len() > prepared.len {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared add capacity is {} value(s), got {}",
                prepared.len,
                out.len()
            )));
        }
        self.read_output_buffer(&prepared.out, out)
    }

    pub fn dispatch_prepared_matmul(
        &mut self,
        prepared: &PreparedMatmulBuffers,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        self.dispatch_prepared_matmul_shape(prepared, prepared.rows, prepared.cols, out)
    }

    pub fn dispatch_prepared_matmul_shape(
        &mut self,
        prepared: &PreparedMatmulBuffers,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul output expected {} value(s), got {}",
                expected,
                out.len()
            )));
        }
        if out.is_empty() {
            return Ok(GpuReadbackUsage::default());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-matmul-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.matmul_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
        self.queue.submit(Some(encoder.finish()));
        self.read_staging_buffer(out)?;
        Ok(readback)
    }

    pub fn submit_prepared_matmul_shape_without_readback(
        &self,
        prepared: &PreparedMatmulBuffers,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        if rows == 0 || cols == 0 {
            return Ok(());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-matmul-submit-only-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-submit-only-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.matmul_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    pub fn read_prepared_matmul_shape_output(
        &mut self,
        prepared: &PreparedMatmulBuffers,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul output expected {} value(s), got {}",
                expected,
                out.len()
            )));
        }
        self.read_output_buffer(&prepared.out, out)
    }

    pub fn dispatch_prepared_matmul_bias_add(
        &mut self,
        prepared: &PreparedMatmulBiasAddBuffers,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        self.dispatch_prepared_matmul_bias_add_shape(
            prepared,
            prepared.matmul.rows,
            prepared.matmul.cols,
            out,
        )
    }

    pub fn dispatch_prepared_matmul_bias_add_shape(
        &mut self,
        prepared: &PreparedMatmulBiasAddBuffers,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.matmul.rows, prepared.matmul.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add output expected {} value(s), got {}",
                expected,
                out.len()
            )));
        }
        if out.is_empty() {
            return Ok(GpuReadbackUsage::default());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-matmul-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.matmul_pipeline);
            pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-bias-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.bias_add_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
        self.queue.submit(Some(encoder.finish()));
        self.read_staging_buffer(out)?;
        Ok(readback)
    }

    pub fn submit_prepared_matmul_bias_add_shape_without_readback(
        &self,
        prepared: &PreparedMatmulBiasAddBuffers,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.matmul.rows, prepared.matmul.cols, rows, cols
            )));
        }
        if rows == 0 || cols == 0 {
            return Ok(());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-matmul-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.matmul_pipeline);
            pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-bias-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.bias_add_pipeline);
            pass.set_bind_group(0, &prepared.bind_group, &[]);
            pass.dispatch_workgroups(
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
                1,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    pub fn read_prepared_matmul_bias_add_shape_output(
        &mut self,
        prepared: &PreparedMatmulBiasAddBuffers,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.matmul.rows, prepared.matmul.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matmul-bias-add output expected {} value(s), got {}",
                expected,
                out.len()
            )));
        }
        self.read_output_buffer(&prepared.out, out)
    }

    fn dispatch_matmul(
        &mut self,
        lhs: &[f32],
        rhs: &[f32],
        out: &mut [f32],
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        let params = MatmulParams {
            rows: checked_u32(rows)?,
            shared: checked_u32(shared)?,
            cols: checked_u32(cols)?,
            _pad: 0,
        };
        let pipeline = self.matmul_pipeline.clone();
        self.dispatch(
            &pipeline,
            &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
            bytemuck::bytes_of(&params),
            out,
            checked_u32(cols.div_ceil(16))?,
            checked_u32(rows.div_ceil(16))?,
        )
    }

    fn dispatch_add(
        &mut self,
        lhs: &[f32],
        rhs: &[f32],
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        let (groups_x, groups_y) = add_groups(out.len());
        let params = AddParams {
            len: checked_u32(out.len())?,
            x_threads: checked_u32(groups_x * 256)?,
            _pad1: 0,
            _pad2: 0,
        };
        let pipeline = self.add_pipeline.clone();
        self.dispatch(
            &pipeline,
            &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
            bytemuck::bytes_of(&params),
            out,
            checked_u32(groups_x)?,
            checked_u32(groups_y)?,
        )
    }

    fn dispatch(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        input_bytes: &[&[u8]],
        params_bytes: &[u8],
        out: &mut [f32],
        workgroups_x: u32,
        workgroups_y: u32,
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if out.is_empty() {
            return Ok(GpuReadbackUsage::default());
        }
        let lhs = storage_buffer(&self.device, input_bytes[0], "arcweft-math-lhs");
        let rhs = storage_buffer(&self.device, input_bytes[1], "arcweft-math-rhs");
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arcweft-math-out"),
            size: std::mem::size_of_val(out) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = storage_buffer(&self.device, params_bytes, "arcweft-math-params");
        let layout = pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-math-bind-group"),
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
                label: Some("arcweft-math-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arcweft-math-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
        }
        let readback = self.encode_readback(&mut encoder, &out_buffer, out.len());
        self.queue.submit(Some(encoder.finish()));
        self.read_staging_buffer(out)?;
        Ok(readback)
    }

    fn read_output_buffer(
        &mut self,
        source: &wgpu::Buffer,
        out: &mut [f32],
    ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
        if out.is_empty() {
            return Ok(GpuReadbackUsage::default());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arcweft-math-prepared-output-readback-encoder"),
            });
        let readback = self.encode_readback(&mut encoder, source, out.len());
        self.queue.submit(Some(encoder.finish()));
        self.read_staging_buffer(out)?;
        Ok(readback)
    }

    fn encode_readback(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        elements: usize,
    ) -> GpuReadbackUsage {
        if elements == 0 {
            return GpuReadbackUsage::default();
        }
        let byte_len = elements * std::mem::size_of::<f32>();
        let usage = self.ensure_readback_buffer(byte_len);
        let readback = &self
            .readback
            .as_ref()
            .expect("readback buffer was initialized")
            .buffer;
        encoder.copy_buffer_to_buffer(source, 0, readback, 0, byte_len as u64);
        usage
    }

    fn ensure_readback_buffer(&mut self, byte_len: usize) -> GpuReadbackUsage {
        match self.readback.as_ref() {
            Some(buffer) if buffer.byte_len >= byte_len => GpuReadbackUsage {
                created: false,
                reused: true,
            },
            _ => {
                self.readback = Some(ReusableReadbackBuffer {
                    buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("arcweft-math-readback"),
                        size: byte_len.max(std::mem::size_of::<f32>()) as u64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    byte_len,
                });
                GpuReadbackUsage {
                    created: true,
                    reused: false,
                }
            }
        }
    }

    fn read_staging_buffer(&self, out: &mut [f32]) -> Result<(), RuntimeMathAcceleratorError> {
        if out.is_empty() {
            return Ok(());
        }
        let byte_len = std::mem::size_of_val(out);
        let buffer = &self
            .readback
            .as_ref()
            .expect("readback buffer was initialized")
            .buffer;
        let (sender, receiver) = mpsc::channel();
        buffer
            .slice(0..byte_len as u64)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?
            .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
        {
            let mapped = buffer.slice(0..byte_len as u64).get_mapped_range();
            let values: &[f32] = bytemuck::cast_slice(&mapped);
            out.copy_from_slice(values);
        }
        buffer.unmap();
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

fn storage_buffer_capacity(
    device: &wgpu::Device,
    byte_len: usize,
    label: &'static str,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: byte_len.max(std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
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

fn checked_u32(value: usize) -> Result<u32, RuntimeMathAcceleratorError> {
    u32::try_from(value).map_err(|_| {
        RuntimeMathAcceleratorError::Backend(format!("GPU dispatch dimension {value} exceeds u32"))
    })
}

fn add_groups(len: usize) -> (usize, usize) {
    let groups = len.div_ceil(256).max(1);
    let groups_x = groups.min(65_535);
    let groups_y = groups.div_ceil(groups_x);
    (groups_x, groups_y.max(1))
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
