use super::compile::{exact_i64_result, runtime_value_kind};
use super::{
    DenseMatrixF32, DenseTensorF32, MatrixBinaryShapeSignature, MatrixBinaryValueSignature,
    MatrixMatmulBiasShapeSignature, MatrixMatmulBiasValueSignature, PreparedMatrixAddCache,
    PreparedMatrixMatmulBiasAddCache, PreparedMatrixMatmulCache, PreparedTensorAddCache,
    RuntimeCallTarget, RuntimeEvalError, RuntimeExternalCallBackend, RuntimeExternalCallContext,
    RuntimeI64Args, RuntimePureAccelerator, RuntimePureHelperRef, RuntimeValue,
    TensorBinaryShapeSignature, TensorBinaryValueSignature, VmPureFunctionScratch, fmt, math,
    runtime_sequence_dense_usize,
};

#[path = "external_data.rs"]
mod external_data;
use external_data::call_data_external;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeAcceleratorExternalCall {
    InferMatmulF32,
    InferAddF32,
    InferBiasAddF32,
    InferMatmulBiasAddF32,
    Conv2dValidF32,
    InferReluF32,
    InferMaxPool2dF32,
    InferSoftmaxLastDimF32,
    InferArgmaxLastDimF32,
    InferFlattenOuterF32,
}

impl RuntimeAcceleratorExternalCall {
    fn from_label(label: &str) -> Option<Self> {
        match label {
            "infer.matmul_f32" => Some(Self::InferMatmulF32),
            "infer.add_f32" => Some(Self::InferAddF32),
            "infer.bias_add_f32" => Some(Self::InferBiasAddF32),
            "infer.matmul_bias_add_f32" => Some(Self::InferMatmulBiasAddF32),
            "conv2d.valid_f32" => Some(Self::Conv2dValidF32),
            "infer.relu_f32" => Some(Self::InferReluF32),
            "infer.max_pool2d_f32" => Some(Self::InferMaxPool2dF32),
            "infer.softmax_last_dim_f32" => Some(Self::InferSoftmaxLastDimF32),
            "infer.argmax_last_dim_f32" => Some(Self::InferArgmaxLastDimF32),
            "infer.flatten_outer_f32" => Some(Self::InferFlattenOuterF32),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::InferMatmulF32 => "infer.matmul_f32",
            Self::InferAddF32 => "infer.add_f32",
            Self::InferBiasAddF32 => "infer.bias_add_f32",
            Self::InferMatmulBiasAddF32 => "infer.matmul_bias_add_f32",
            Self::Conv2dValidF32 => "conv2d.valid_f32",
            Self::InferReluF32 => "infer.relu_f32",
            Self::InferMaxPool2dF32 => "infer.max_pool2d_f32",
            Self::InferSoftmaxLastDimF32 => "infer.softmax_last_dim_f32",
            Self::InferArgmaxLastDimF32 => "infer.argmax_last_dim_f32",
            Self::InferFlattenOuterF32 => "infer.flatten_outer_f32",
        }
    }
}

fn data_runtime_error(name: impl Into<String>, reason: impl Into<String>) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: name.into(),
        reason: reason.into(),
    }
}

fn runtime_value_label_for_data(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::String(value) => format!("string/{value}"),
        RuntimeValue::Char(value) => format!("char/{value}"),
        RuntimeValue::Seq(seq) => format!("seq/{}", seq.len()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::NominalRecord(record) => {
            format!("nominal-record/{}", record.type_id().as_str())
        }
        RuntimeValue::Range(range) => range.label(),
        RuntimeValue::Agent(value) => value.label().to_owned(),
        RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            ..
        } => format!("variant/{owner:?}/#{ordinal}/{name}"),
        RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::ProjectContinuation(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => "non-data runtime value".to_owned(),
    }
}

impl RuntimeExternalCallBackend for RuntimePureAccelerator {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        if let Some(result) = call_data_external(self, context, callee.as_label(), args) {
            return Some(result);
        }
        let call = RuntimeAcceleratorExternalCall::from_label(callee.as_label())?;
        Some(match (call, args) {
            (
                RuntimeAcceleratorExternalCall::InferMatmulF32,
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => self
                .call_infer_matmul_f32(lhs, rhs)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferAddF32,
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => self
                .call_infer_add_f32(lhs, rhs)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferBiasAddF32,
                [
                    RuntimeValue::TensorF32(tensor),
                    RuntimeValue::TensorF32(bias),
                ],
            ) => self
                .call_infer_bias_add_f32(tensor, bias)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferMatmulBiasAddF32,
                [
                    RuntimeValue::TensorF32(lhs),
                    RuntimeValue::TensorF32(rhs),
                    RuntimeValue::TensorF32(bias),
                ],
            ) => self
                .call_infer_matmul_bias_add_f32(lhs, rhs, bias)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::Conv2dValidF32,
                [
                    RuntimeValue::TensorF32(input),
                    RuntimeValue::TensorF32(kernel),
                    stride_y,
                    stride_x,
                ],
            ) => runtime_value_to_usize(call, stride_y).and_then(|stride_y| {
                runtime_value_to_usize(call, stride_x).and_then(|stride_x| {
                    self.call_conv2d_valid_f32(input, kernel, stride_y, stride_x)
                        .map(RuntimeValue::tensor_f32)
                })
            }),
            (RuntimeAcceleratorExternalCall::InferReluF32, [RuntimeValue::TensorF32(input)]) => {
                self.call_infer_relu_f32(input)
                    .map(RuntimeValue::tensor_f32)
            }
            (
                RuntimeAcceleratorExternalCall::InferMaxPool2dF32,
                [
                    RuntimeValue::TensorF32(input),
                    kernel_y,
                    kernel_x,
                    stride_y,
                    stride_x,
                ],
            ) => runtime_value_to_usize(call, kernel_y).and_then(|kernel_y| {
                runtime_value_to_usize(call, kernel_x).and_then(|kernel_x| {
                    runtime_value_to_usize(call, stride_y).and_then(|stride_y| {
                        runtime_value_to_usize(call, stride_x).and_then(|stride_x| {
                            self.call_infer_max_pool2d_f32(
                                input, kernel_y, kernel_x, stride_y, stride_x,
                            )
                            .map(RuntimeValue::tensor_f32)
                        })
                    })
                })
            }),
            (
                RuntimeAcceleratorExternalCall::InferSoftmaxLastDimF32,
                [RuntimeValue::TensorF32(input)],
            ) => self
                .call_infer_softmax_last_dim_f32(input)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferArgmaxLastDimF32,
                [RuntimeValue::TensorF32(input)],
            ) => Ok(runtime_class_indices_value(
                self.call_infer_argmax_last_dim_f32(input),
            )),
            (
                RuntimeAcceleratorExternalCall::InferFlattenOuterF32,
                [RuntimeValue::TensorF32(input)],
            ) => self
                .call_infer_flatten_outer_f32(input)
                .map(RuntimeValue::tensor_f32),
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: call.label().to_owned(),
                reason: "argument shape is not supported by this adapter call".to_owned(),
            }),
        })
    }
}

fn runtime_class_indices_value(indices: Vec<usize>) -> RuntimeValue {
    runtime_sequence_dense_usize(
        indices
            .into_iter()
            .map(|index| u64::try_from(index).unwrap_or(u64::MAX))
            .collect(),
    )
}

fn runtime_value_to_usize(
    call: RuntimeAcceleratorExternalCall,
    value: &RuntimeValue,
) -> Result<usize, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok()),
        RuntimeValue::UInt(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok()),
        _ => None,
    }
    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
        name: call.label().to_owned(),
        reason: format!("expected usize-compatible integer, got {value:?}"),
    })
}

pub(super) fn infer_runtime_error(name: &str, error: impl fmt::Display) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: name.to_owned(),
        reason: error.to_string(),
    }
}

impl RuntimePureAccelerator {
    pub(super) fn cache_entries(&self) -> usize {
        self.cache.iter().filter(|entry| entry.is_some()).count()
    }

    pub(super) fn call_runtime_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.matmul_backend_selection(lhs, rhs);
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matmul_f32(lhs, rhs);
        }
        if lhs.cols() != rhs.rows() {
            return self.math.matmul_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matmul.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_matrix_matmul_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            self.math.run_prepared_matrix_matmul_f32_shape_into(
                &cache.prepared,
                lhs.rows(),
                rhs.cols(),
                &mut out,
            )?;
            let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matmul = Some(cache);
            return result;
        }
        let capacity_signature = MatrixBinaryShapeSignature::capacity_for_matmul(lhs, rhs);
        let prepared = self.math.prepare_matrix_matmul_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
            capacity_signature.rhs.cols,
        )?;
        self.math
            .update_prepared_matrix_matmul_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        self.math.run_prepared_matrix_matmul_f32_shape_into(
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut out,
        )?;
        let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matmul = Some(PreparedMatrixMatmulCache {
            signature,
            capacity_signature,
            value_signature: MatrixBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_matmul_bias_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.matmul_backend_selection(lhs, rhs);
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matmul_bias_add_f32(lhs, rhs, bias);
        }
        if lhs.cols() != rhs.rows() || bias.shape().dims() != [rhs.cols()] {
            return self.math.matmul_bias_add_f32(lhs, rhs, bias);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixMatmulBiasShapeSignature::new(lhs, rhs, bias);
        if let Some(cache) = self.math_prepare_cache.matmul_bias_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs, bias) {
                self.math.update_prepared_matrix_matmul_bias_add_f32(
                    &cache.prepared,
                    lhs,
                    rhs,
                    bias,
                )?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs, bias);
            }
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            self.math
                .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                    &cache.prepared,
                    lhs.rows(),
                    rhs.cols(),
                    &mut out,
                )?;
            let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matmul_bias_add = Some(cache);
            return result;
        }
        let capacity_signature = MatrixMatmulBiasShapeSignature::capacity_for(lhs, rhs, bias);
        let prepared = self.math.prepare_matrix_matmul_bias_add_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
            capacity_signature.rhs.cols,
        )?;
        self.math
            .update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        self.math
            .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                &prepared,
                lhs.rows(),
                rhs.cols(),
                &mut out,
            )?;
        let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matmul_bias_add = Some(PreparedMatrixMatmulBiasAddCache {
            signature,
            capacity_signature,
            value_signature: MatrixMatmulBiasValueSignature::new(lhs, rhs, bias),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.elementwise_backend_selection(lhs.values().len());
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matrix_add_f32(lhs, rhs);
        }
        if lhs.shape() != rhs.shape() {
            return self.math.matrix_add_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matrix_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_matrix_add_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.math.run_prepared_matrix_add_f32_shape_into(
                &cache.prepared,
                lhs.rows(),
                lhs.cols(),
                &mut out,
            )?;
            let result = DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matrix_add = Some(cache);
            return result;
        }
        let capacity_signature = MatrixBinaryShapeSignature::capacity_for_matrix_add(lhs, rhs);
        let prepared = self.math.prepare_matrix_add_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
        )?;
        self.math
            .update_prepared_matrix_add_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.values().len()];
        self.math.run_prepared_matrix_add_f32_shape_into(
            &prepared,
            lhs.rows(),
            lhs.cols(),
            &mut out,
        )?;
        let result = DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matrix_add = Some(PreparedMatrixAddCache {
            signature,
            capacity_signature,
            value_signature: MatrixBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.elementwise_backend_selection(lhs.values().len());
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.tensor_add_f32(lhs, rhs);
        }
        if lhs.shape() != rhs.shape() {
            return self.math.tensor_add_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = TensorBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.tensor_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_tensor_add_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.math.run_prepared_tensor_add_f32_len_into(
                &cache.prepared,
                lhs.values().len(),
                &mut out,
            )?;
            let result = DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into);
            self.math_prepare_cache.tensor_add = Some(cache);
            return result;
        }
        let capacity_signature = TensorBinaryShapeSignature::capacity_for_add(lhs, rhs);
        let prepared = self
            .math
            .prepare_tensor_add_f32_capacity(capacity_signature.lhs.element_count())?;
        self.math
            .update_prepared_tensor_add_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.values().len()];
        self.math
            .run_prepared_tensor_add_f32_len_into(&prepared, lhs.values().len(), &mut out)?;
        let result = DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into);
        self.math_prepare_cache.tensor_add = Some(PreparedTensorAddCache {
            signature,
            capacity_signature,
            value_signature: TensorBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn record_math_inputs<T>(&mut self, lhs_elements: usize, rhs_elements: usize) {
        self.stats.arg_bytes_borrowed +=
            lhs_elements.saturating_add(rhs_elements) * std::mem::size_of::<T>();
    }

    pub(super) fn record_math_result<T>(&mut self, result_elements: usize) {
        self.stats.result_bytes_copied += result_elements * std::mem::size_of::<T>();
        if !matches!(
            self.math.stats().last_backend,
            Some(math::RuntimeMathBackend::Scalar) | None
        ) {
            self.stats.math_accelerated_calls += 1;
        }
    }

    pub(super) fn call_vm_i64(
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI64Args,
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_args(helper.plan(), helper.id(), args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_i64_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[i64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_slice(helper.plan(), helper.id(), args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_i32_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i32, RuntimeEvalError> {
        match scratch.evaluate_i32_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::Int(value) => {
                value
                    .exact_i32()
                    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: format!("pure i32 result `{value}` is outside i32 range"),
                    })
            }
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_f32_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[f32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f32, RuntimeEvalError> {
        match scratch.evaluate_f32_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::F32(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f32 result expected f32, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }

    pub(super) fn call_vm_f64_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[f64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f64, RuntimeEvalError> {
        match scratch.evaluate_f64_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::F64(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f64 result expected f64, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }
}
