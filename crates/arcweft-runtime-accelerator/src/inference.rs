//! Deterministic forward inference graph for dense `f32` tensors.

use crate::math::{
    RuntimeMathAccelerator, RuntimeMathAcceleratorError, RuntimePreparedMatrixMatmulBiasAddF32,
};
use crate::{f32_value_bits, f32_value_bits_match, power_of_two_capacity, update_f32_value_bits};
use arcweft_core::math::{DenseMatrixF32, DenseTensorF32, RuntimeMathError};
use std::collections::BTreeMap;
use thiserror::Error;

/// Stable tensor handle inside an inference graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InferenceTensorId(usize);

impl InferenceTensorId {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Dense tensor shape used by forward graph construction and validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceShape {
    dims: Vec<usize>,
}

impl InferenceShape {
    pub fn new(dims: Vec<usize>) -> Result<Self, InferenceError> {
        if dims.is_empty() {
            return Err(InferenceError::InvalidRank { rank: 0 });
        }
        if dims.contains(&0) {
            return Err(InferenceError::InvalidZeroDimension);
        }
        let _ = checked_product(&dims)?;
        Ok(Self { dims })
    }

    pub fn matrix(rows: usize, cols: usize) -> Result<Self, InferenceError> {
        Self::new(vec![rows, cols])
    }

    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    pub fn element_count(&self) -> usize {
        self.dims.iter().product()
    }

    fn last_dim(&self) -> usize {
        *self.dims.last().expect("shape rank is non-zero")
    }
}

/// One named tensor input supplied at inference time.
#[derive(Clone, Debug)]
pub struct InferenceInput {
    id: InferenceTensorId,
    name: String,
    shape: InferenceShape,
}

impl InferenceInput {
    pub fn id(&self) -> InferenceTensorId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shape(&self) -> &InferenceShape {
        &self.shape
    }
}

/// Supported forward-only tensor operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceOp {
    Matmul,
    Add,
    BiasAdd,
    Conv2dValid {
        stride_y: usize,
        stride_x: usize,
    },
    Relu,
    MaxPool2d {
        kernel_y: usize,
        kernel_x: usize,
        stride_y: usize,
        stride_x: usize,
    },
    SoftmaxLastDim,
    ArgmaxLastDim,
    FlattenOuter,
}

/// Inference output value.
#[derive(Clone, Debug, PartialEq)]
pub enum InferenceValue {
    Tensor(DenseTensorF32),
    ClassIndices(Vec<usize>),
}

impl InferenceValue {
    pub fn as_tensor(&self) -> Option<&DenseTensorF32> {
        match self {
            Self::Tensor(tensor) => Some(tensor),
            Self::ClassIndices(_) => None,
        }
    }

    pub fn as_class_indices(&self) -> Option<&[usize]> {
        match self {
            Self::Tensor(_) => None,
            Self::ClassIndices(indices) => Some(indices),
        }
    }
}

/// Fully validated forward graph.
#[derive(Clone, Debug)]
pub struct InferenceGraph {
    tensors: Vec<TensorSpec>,
    nodes: Vec<InferenceNode>,
    inputs: Vec<InferenceTensorId>,
    outputs: Vec<InferenceTensorId>,
}

impl InferenceGraph {
    pub fn builder() -> InferenceGraphBuilder {
        InferenceGraphBuilder::default()
    }

    pub fn inputs(&self) -> impl Iterator<Item = &InferenceInput> {
        self.inputs
            .iter()
            .map(|id| match &self.tensors[id.index()] {
                TensorSpec::Input(input) => input,
                TensorSpec::Constant { .. } | TensorSpec::NodeOutput { .. } => {
                    unreachable!("input list only contains input tensors")
                }
            })
    }

    pub fn outputs(&self) -> &[InferenceTensorId] {
        &self.outputs
    }

    pub fn tensor_shape(&self, id: InferenceTensorId) -> Option<&InferenceShape> {
        self.tensors.get(id.index()).map(TensorSpec::shape)
    }

    pub fn tensor_name(&self, id: InferenceTensorId) -> Option<&str> {
        self.tensors.get(id.index()).and_then(TensorSpec::name)
    }

    fn tensor_use_counts(&self) -> Vec<usize> {
        let mut uses = vec![0_usize; self.tensors.len()];
        for node in &self.nodes {
            for input in &node.inputs {
                uses[input.index()] += 1;
            }
        }
        for output in &self.outputs {
            uses[output.index()] += 1;
        }
        uses
    }
}

/// Builder for a validated topological inference graph.
#[derive(Default)]
pub struct InferenceGraphBuilder {
    tensors: Vec<TensorSpec>,
    nodes: Vec<InferenceNode>,
    inputs: Vec<InferenceTensorId>,
    outputs: Vec<InferenceTensorId>,
}

impl InferenceGraphBuilder {
    pub fn add_input(
        &mut self,
        name: impl Into<String>,
        shape: InferenceShape,
    ) -> InferenceTensorId {
        let id = InferenceTensorId(self.tensors.len());
        self.tensors.push(TensorSpec::Input(InferenceInput {
            id,
            name: name.into(),
            shape,
        }));
        self.inputs.push(id);
        id
    }

    pub fn add_constant(
        &mut self,
        name: impl Into<String>,
        tensor: DenseTensorF32,
    ) -> Result<InferenceTensorId, InferenceError> {
        let shape = InferenceShape::new(tensor.shape().dims().to_vec())?;
        let id = InferenceTensorId(self.tensors.len());
        self.tensors.push(TensorSpec::Constant {
            name: name.into(),
            shape,
            tensor,
        });
        Ok(id)
    }

    pub fn add_matmul(
        &mut self,
        lhs: InferenceTensorId,
        rhs: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let lhs_shape = self.shape(lhs)?;
        let rhs_shape = self.shape(rhs)?;
        let [rows, shared_lhs] = matrix_dims(lhs_shape, InferenceOp::Matmul)?;
        let [shared_rhs, cols] = matrix_dims(rhs_shape, InferenceOp::Matmul)?;
        if shared_lhs != shared_rhs {
            return Err(InferenceError::ShapeMismatch {
                op: InferenceOp::Matmul,
                lhs: lhs_shape.clone(),
                rhs: rhs_shape.clone(),
            });
        }
        self.add_node(
            InferenceOp::Matmul,
            vec![lhs, rhs],
            InferenceShape::matrix(rows, cols)?,
        )
    }

    pub fn add_add(
        &mut self,
        lhs: InferenceTensorId,
        rhs: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let lhs_shape = self.shape(lhs)?;
        let rhs_shape = self.shape(rhs)?;
        if lhs_shape != rhs_shape {
            return Err(InferenceError::ShapeMismatch {
                op: InferenceOp::Add,
                lhs: lhs_shape.clone(),
                rhs: rhs_shape.clone(),
            });
        }
        self.add_node(InferenceOp::Add, vec![lhs, rhs], lhs_shape.clone())
    }

    pub fn add_bias_add(
        &mut self,
        tensor: InferenceTensorId,
        bias: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let tensor_shape = self.shape(tensor)?;
        let bias_shape = self.shape(bias)?;
        if bias_shape.rank() != 1 || bias_shape.last_dim() != tensor_shape.last_dim() {
            return Err(InferenceError::BiasShapeMismatch {
                tensor: tensor_shape.clone(),
                bias: bias_shape.clone(),
            });
        }
        self.add_node(
            InferenceOp::BiasAdd,
            vec![tensor, bias],
            tensor_shape.clone(),
        )
    }

    pub fn add_conv2d_valid(
        &mut self,
        input: InferenceTensorId,
        kernel: InferenceTensorId,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<InferenceTensorId, InferenceError> {
        validate_window(stride_y, stride_x)?;
        let input_shape = self.shape(input)?;
        let kernel_shape = self.shape(kernel)?;
        let [batch, input_channels, height, width] =
            nchw_dims(input_shape, InferenceOp::Conv2dValid { stride_y, stride_x })?;
        let [
            output_channels,
            kernel_channels,
            kernel_height,
            kernel_width,
        ] = oihw_dims(
            kernel_shape,
            InferenceOp::Conv2dValid { stride_y, stride_x },
        )?;
        if input_channels != kernel_channels || height < kernel_height || width < kernel_width {
            return Err(InferenceError::Conv2dShapeMismatch {
                input: input_shape.clone(),
                kernel: kernel_shape.clone(),
            });
        }
        let output_height = (height - kernel_height) / stride_y + 1;
        let output_width = (width - kernel_width) / stride_x + 1;
        self.add_node(
            InferenceOp::Conv2dValid { stride_y, stride_x },
            vec![input, kernel],
            InferenceShape::new(vec![batch, output_channels, output_height, output_width])?,
        )
    }

    pub fn add_relu(
        &mut self,
        input: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let shape = self.shape(input)?.clone();
        self.add_node(InferenceOp::Relu, vec![input], shape)
    }

    pub fn add_max_pool2d(
        &mut self,
        input: InferenceTensorId,
        kernel_y: usize,
        kernel_x: usize,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<InferenceTensorId, InferenceError> {
        validate_window(kernel_y, kernel_x)?;
        validate_window(stride_y, stride_x)?;
        let input_shape = self.shape(input)?;
        let [batch, channels, height, width] = nchw_dims(
            input_shape,
            InferenceOp::MaxPool2d {
                kernel_y,
                kernel_x,
                stride_y,
                stride_x,
            },
        )?;
        if height < kernel_y || width < kernel_x {
            return Err(InferenceError::PoolingShapeMismatch {
                input: input_shape.clone(),
                kernel_y,
                kernel_x,
            });
        }
        let output_height = (height - kernel_y) / stride_y + 1;
        let output_width = (width - kernel_x) / stride_x + 1;
        self.add_node(
            InferenceOp::MaxPool2d {
                kernel_y,
                kernel_x,
                stride_y,
                stride_x,
            },
            vec![input],
            InferenceShape::new(vec![batch, channels, output_height, output_width])?,
        )
    }

    pub fn add_softmax_last_dim(
        &mut self,
        input: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let shape = self.shape(input)?.clone();
        self.add_node(InferenceOp::SoftmaxLastDim, vec![input], shape)
    }

    pub fn add_argmax_last_dim(
        &mut self,
        input: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let shape = self.shape(input)?;
        if shape.last_dim() == 0 {
            return Err(InferenceError::InvalidZeroDimension);
        }
        let out_shape = if shape.rank() == 1 {
            InferenceShape::new(vec![1])?
        } else {
            InferenceShape::new(shape.dims[..shape.rank() - 1].to_vec())?
        };
        self.add_node(InferenceOp::ArgmaxLastDim, vec![input], out_shape)
    }

    pub fn add_flatten_outer(
        &mut self,
        input: InferenceTensorId,
    ) -> Result<InferenceTensorId, InferenceError> {
        let shape = self.shape(input)?;
        if shape.rank() == 1 {
            return self.add_node(
                InferenceOp::FlattenOuter,
                vec![input],
                InferenceShape::matrix(1, shape.element_count())?,
            );
        }
        let outer = shape.dims[0];
        let inner = checked_product(&shape.dims[1..])?;
        self.add_node(
            InferenceOp::FlattenOuter,
            vec![input],
            InferenceShape::matrix(outer, inner)?,
        )
    }

    pub fn set_outputs<I>(&mut self, outputs: I) -> Result<(), InferenceError>
    where
        I: IntoIterator<Item = InferenceTensorId>,
    {
        let outputs = outputs.into_iter().collect::<Vec<_>>();
        for output in &outputs {
            let _ = self.shape(*output)?;
        }
        self.outputs = outputs;
        Ok(())
    }

    pub fn build(self) -> Result<InferenceGraph, InferenceError> {
        if self.outputs.is_empty() {
            return Err(InferenceError::NoOutputs);
        }
        Ok(InferenceGraph {
            tensors: self.tensors,
            nodes: self.nodes,
            inputs: self.inputs,
            outputs: self.outputs,
        })
    }

    fn add_node(
        &mut self,
        op: InferenceOp,
        inputs: Vec<InferenceTensorId>,
        shape: InferenceShape,
    ) -> Result<InferenceTensorId, InferenceError> {
        for input in &inputs {
            let _ = self.shape(*input)?;
        }
        let output = InferenceTensorId(self.tensors.len());
        self.tensors.push(TensorSpec::NodeOutput { shape });
        self.nodes.push(InferenceNode { op, inputs, output });
        Ok(output)
    }

    fn shape(&self, id: InferenceTensorId) -> Result<&InferenceShape, InferenceError> {
        self.tensors
            .get(id.index())
            .map(TensorSpec::shape)
            .ok_or(InferenceError::UnknownTensor { id })
    }
}

/// Adapter boundary for forward-only inference execution.
pub trait InferenceAdapter {
    fn matmul(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn add(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn bias_add(
        &mut self,
        tensor: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn matmul_bias_add(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        let product = self.matmul(lhs, rhs)?;
        self.bias_add(&product, bias)
    }

    fn conv2d_valid(
        &mut self,
        input: &DenseTensorF32,
        kernel: &DenseTensorF32,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn relu(&mut self, input: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError>;

    fn max_pool2d(
        &mut self,
        input: &DenseTensorF32,
        kernel_y: usize,
        kernel_x: usize,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn softmax_last_dim(
        &mut self,
        input: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError>;

    fn argmax_last_dim(&mut self, input: &DenseTensorF32) -> Vec<usize>;

    fn flatten_outer(&mut self, input: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError>;
}

/// Default inference adapter backed by `RuntimeMathAccelerator` for dense matmul.
pub struct AcceleratedInferenceAdapter {
    accelerator: RuntimeMathAccelerator,
    matmul_bias_add_cache: Option<PreparedInferenceMatmulBiasAddCache>,
}

impl AcceleratedInferenceAdapter {
    pub fn new(accelerator: RuntimeMathAccelerator) -> Self {
        Self {
            accelerator,
            matmul_bias_add_cache: None,
        }
    }

    pub fn accelerator(&self) -> &RuntimeMathAccelerator {
        &self.accelerator
    }

    pub fn accelerator_mut(&mut self) -> &mut RuntimeMathAccelerator {
        &mut self.accelerator
    }

    fn matmul_bias_add_cached(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        let selection = self.accelerator.matmul_backend_selection(lhs, rhs);
        if selection.backend() != crate::math::RuntimeMathBackend::Wgpu {
            return self.accelerator.matmul_bias_add_f32(lhs, rhs, bias);
        }
        if lhs.cols() != rhs.rows() || bias.shape().dims() != [rhs.cols()] {
            return self.accelerator.matmul_bias_add_f32(lhs, rhs, bias);
        }
        self.accelerator.record_backend_selection(selection);
        let signature = InferenceMatmulBiasShapeSignature::new(lhs, rhs, bias);
        if let Some(cache) = self.matmul_bias_add_cache.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs, bias) {
                self.accelerator
                    .update_prepared_matrix_matmul_bias_add_f32(&cache.prepared, lhs, rhs, bias)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs, bias);
            }
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            self.accelerator
                .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                    &cache.prepared,
                    lhs.rows(),
                    rhs.cols(),
                    &mut out,
                )?;
            let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
            self.matmul_bias_add_cache = Some(cache);
            return result;
        }
        let capacity_signature = InferenceMatmulBiasShapeSignature::capacity_for(lhs, rhs, bias);
        let prepared = self
            .accelerator
            .prepare_matrix_matmul_bias_add_f32_capacity(
                capacity_signature.lhs.rows,
                capacity_signature.lhs.cols,
                capacity_signature.rhs.cols,
            )?;
        self.accelerator
            .update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        self.accelerator
            .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                &prepared,
                lhs.rows(),
                rhs.cols(),
                &mut out,
            )?;
        let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
        self.matmul_bias_add_cache = Some(PreparedInferenceMatmulBiasAddCache {
            signature,
            capacity_signature,
            value_signature: InferenceMatmulBiasValueSignature::new(lhs, rhs, bias),
            prepared,
        });
        result
    }
}

struct PreparedInferenceMatmulBiasAddCache {
    signature: InferenceMatmulBiasShapeSignature,
    capacity_signature: InferenceMatmulBiasShapeSignature,
    value_signature: InferenceMatmulBiasValueSignature,
    prepared: RuntimePreparedMatrixMatmulBiasAddF32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InferenceMatmulBiasShapeSignature {
    lhs: InferenceMatrixShapeSignature,
    rhs: InferenceMatrixShapeSignature,
    bias: InferenceTensorShapeSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InferenceMatrixShapeSignature {
    rows: usize,
    cols: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InferenceTensorShapeSignature {
    element_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InferenceMatmulBiasValueSignature {
    lhs: Vec<u32>,
    rhs: Vec<u32>,
    bias: Vec<u32>,
}

impl InferenceMatmulBiasShapeSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: InferenceMatrixShapeSignature::new(lhs),
            rhs: InferenceMatrixShapeSignature::new(rhs),
            bias: InferenceTensorShapeSignature::new(bias),
        }
    }

    fn capacity_for(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: InferenceMatrixShapeSignature::capacity_for(lhs),
            rhs: InferenceMatrixShapeSignature::capacity_for(rhs),
            bias: InferenceTensorShapeSignature::capacity_for(bias),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.lhs.contains(&shape.lhs)
            && self.rhs.contains(&shape.rhs)
            && self.bias.contains(&shape.bias)
    }
}

impl InferenceMatrixShapeSignature {
    fn new(matrix: &DenseMatrixF32) -> Self {
        Self {
            rows: matrix.rows(),
            cols: matrix.cols(),
        }
    }

    fn capacity_for(matrix: &DenseMatrixF32) -> Self {
        Self {
            rows: power_of_two_capacity(matrix.rows()),
            cols: power_of_two_capacity(matrix.cols()),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.rows >= shape.rows && self.cols >= shape.cols
    }
}

impl InferenceTensorShapeSignature {
    fn new(tensor: &DenseTensorF32) -> Self {
        Self {
            element_count: tensor.values().len(),
        }
    }

    fn capacity_for(tensor: &DenseTensorF32) -> Self {
        Self {
            element_count: power_of_two_capacity(tensor.values().len()),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.element_count >= shape.element_count
    }
}

impl InferenceMatmulBiasValueSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: f32_value_bits(lhs.values()),
            rhs: f32_value_bits(rhs.values()),
            bias: f32_value_bits(bias.values()),
        }
    }

    fn matches(&self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> bool {
        f32_value_bits_match(&self.lhs, lhs.values())
            && f32_value_bits_match(&self.rhs, rhs.values())
            && f32_value_bits_match(&self.bias, bias.values())
    }

    fn update(&mut self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) {
        update_f32_value_bits(&mut self.lhs, lhs.values());
        update_f32_value_bits(&mut self.rhs, rhs.values());
        update_f32_value_bits(&mut self.bias, bias.values());
    }
}

impl InferenceAdapter for AcceleratedInferenceAdapter {
    fn matmul(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        let lhs = tensor_as_matrix(lhs, InferenceOp::Matmul)?;
        let rhs = tensor_as_matrix(rhs, InferenceOp::Matmul)?;
        let out = self.accelerator.matmul_f32(&lhs, &rhs)?;
        Ok(DenseTensorF32::from_matrix(out))
    }

    fn add(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        lhs.add_scalar(rhs).map_err(Into::into)
    }

    fn bias_add(
        &mut self,
        tensor: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        bias_add(tensor, bias)
    }

    fn matmul_bias_add(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        let lhs = tensor_as_matrix(lhs, InferenceOp::Matmul)?;
        let rhs = tensor_as_matrix(rhs, InferenceOp::Matmul)?;
        let out = self.matmul_bias_add_cached(&lhs, &rhs, bias)?;
        Ok(DenseTensorF32::from_matrix(out))
    }

    fn conv2d_valid(
        &mut self,
        input: &DenseTensorF32,
        kernel: &DenseTensorF32,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, InferenceError> {
        conv2d_valid_nchw(input, kernel, stride_y, stride_x)
    }

    fn relu(&mut self, input: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError> {
        map_tensor(input, |value| value.max(0.0))
    }

    fn max_pool2d(
        &mut self,
        input: &DenseTensorF32,
        kernel_y: usize,
        kernel_x: usize,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, InferenceError> {
        max_pool2d_nchw(input, kernel_y, kernel_x, stride_y, stride_x)
    }

    fn softmax_last_dim(
        &mut self,
        input: &DenseTensorF32,
    ) -> Result<DenseTensorF32, InferenceError> {
        softmax_last_dim(input)
    }

    fn argmax_last_dim(&mut self, input: &DenseTensorF32) -> Vec<usize> {
        argmax_last_dim(input)
    }

    fn flatten_outer(&mut self, input: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError> {
        flatten_outer(input)
    }
}

/// Forward-only inference session using a concrete execution adapter.
pub struct InferenceSession<A = AcceleratedInferenceAdapter>
where
    A: InferenceAdapter,
{
    graph: InferenceGraph,
    adapter: A,
}

impl<A> InferenceSession<A>
where
    A: InferenceAdapter,
{
    pub fn new(graph: InferenceGraph, adapter: A) -> Self {
        Self { graph, adapter }
    }

    pub fn graph(&self) -> &InferenceGraph {
        &self.graph
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    pub fn run<I>(&mut self, inputs: I) -> Result<Vec<InferenceValue>, InferenceError>
    where
        I: IntoIterator<Item = (InferenceTensorId, DenseTensorF32)>,
    {
        let supplied = inputs.into_iter().collect::<BTreeMap<_, _>>();
        self.run_borrowed(supplied.iter().map(|(id, tensor)| (*id, tensor)))
    }

    /// Runs the graph with borrowed input tensors.
    ///
    /// Constants and supplied inputs stay borrowed in the per-run value table
    /// until an operation produces an owned tensor, avoiding an extra
    /// input/constant clone at the session boundary.
    pub fn run_borrowed<'a, I>(
        &'a mut self,
        inputs: I,
    ) -> Result<Vec<InferenceValue>, InferenceError>
    where
        I: IntoIterator<Item = (InferenceTensorId, &'a DenseTensorF32)>,
    {
        let supplied = inputs.into_iter().collect::<BTreeMap<_, _>>();
        let mut values = vec![None; self.graph.tensors.len()];
        for (index, spec) in self.graph.tensors.iter().enumerate() {
            if let TensorSpec::Constant { tensor, .. } = spec {
                values[index] = Some(InferenceRunValue::BorrowedTensor(tensor));
            }
        }
        for input in self.graph.inputs() {
            let tensor = supplied
                .get(&input.id())
                .ok_or_else(|| InferenceError::MissingInput {
                    name: input.name().to_owned(),
                })?;
            validate_tensor_shape(tensor, input.shape())?;
            values[input.id().index()] = Some(InferenceRunValue::BorrowedTensor(tensor));
        }
        let tensor_uses = self.graph.tensor_use_counts();
        let nodes = self.graph.nodes.clone();
        let adapter = &mut self.adapter;
        let mut index = 0_usize;
        while let Some(node) = nodes.get(index) {
            if let Some(next_node) = nodes.get(index + 1)
                && Self::can_fuse_matmul_bias_add(node, next_node, &tensor_uses)
            {
                let value = Self::eval_matmul_bias_add(adapter, node, next_node, &values)?;
                values[next_node.output.index()] = Some(value);
                index += 2;
                continue;
            }

            let value = Self::eval_node(adapter, node, &values)?;
            values[node.output.index()] = Some(value);
            index += 1;
        }
        self.graph
            .outputs
            .iter()
            .map(|id| {
                values[id.index()]
                    .as_ref()
                    .map(InferenceRunValue::to_output)
                    .ok_or(InferenceError::UncomputedTensor { id: *id })
            })
            .collect()
    }

    fn can_fuse_matmul_bias_add(
        node: &InferenceNode,
        next_node: &InferenceNode,
        tensor_uses: &[usize],
    ) -> bool {
        matches!(node.op, InferenceOp::Matmul)
            && matches!(next_node.op, InferenceOp::BiasAdd)
            && next_node.inputs.first().copied() == Some(node.output)
            && tensor_uses.get(node.output.index()).copied() == Some(1)
    }

    fn eval_matmul_bias_add(
        adapter: &mut A,
        matmul: &InferenceNode,
        bias_add_node: &InferenceNode,
        values: &[Option<InferenceRunValue<'_>>],
    ) -> Result<InferenceRunValue<'static>, InferenceError> {
        let lhs = tensor_value(values, matmul.inputs[0])?;
        let rhs = tensor_value(values, matmul.inputs[1])?;
        let bias = tensor_value(values, bias_add_node.inputs[1])?;
        Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
            adapter.matmul_bias_add(lhs, rhs, bias)?,
        )))
    }

    fn eval_node(
        adapter: &mut A,
        node: &InferenceNode,
        values: &[Option<InferenceRunValue<'_>>],
    ) -> Result<InferenceRunValue<'static>, InferenceError> {
        match node.op {
            InferenceOp::Matmul => {
                let lhs = tensor_value(values, node.inputs[0])?;
                let rhs = tensor_value(values, node.inputs[1])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.matmul(lhs, rhs)?,
                )))
            }
            InferenceOp::Add => {
                let lhs = tensor_value(values, node.inputs[0])?;
                let rhs = tensor_value(values, node.inputs[1])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.add(lhs, rhs)?,
                )))
            }
            InferenceOp::BiasAdd => {
                let tensor = tensor_value(values, node.inputs[0])?;
                let bias = tensor_value(values, node.inputs[1])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.bias_add(tensor, bias)?,
                )))
            }
            InferenceOp::Conv2dValid { stride_y, stride_x } => {
                let input = tensor_value(values, node.inputs[0])?;
                let kernel = tensor_value(values, node.inputs[1])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.conv2d_valid(input, kernel, stride_y, stride_x)?,
                )))
            }
            InferenceOp::Relu => {
                let input = tensor_value(values, node.inputs[0])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.relu(input)?,
                )))
            }
            InferenceOp::MaxPool2d {
                kernel_y,
                kernel_x,
                stride_y,
                stride_x,
            } => {
                let input = tensor_value(values, node.inputs[0])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.max_pool2d(input, kernel_y, kernel_x, stride_y, stride_x)?,
                )))
            }
            InferenceOp::SoftmaxLastDim => {
                let input = tensor_value(values, node.inputs[0])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.softmax_last_dim(input)?,
                )))
            }
            InferenceOp::ArgmaxLastDim => {
                let input = tensor_value(values, node.inputs[0])?;
                Ok(InferenceRunValue::Owned(InferenceValue::ClassIndices(
                    adapter.argmax_last_dim(input),
                )))
            }
            InferenceOp::FlattenOuter => {
                let input = tensor_value(values, node.inputs[0])?;
                Ok(InferenceRunValue::Owned(InferenceValue::Tensor(
                    adapter.flatten_outer(input)?,
                )))
            }
        }
    }
}

#[derive(Clone, Debug)]
enum InferenceRunValue<'a> {
    BorrowedTensor(&'a DenseTensorF32),
    Owned(InferenceValue),
}

impl InferenceRunValue<'_> {
    fn as_tensor(&self) -> Option<&DenseTensorF32> {
        match self {
            Self::BorrowedTensor(tensor) => Some(tensor),
            Self::Owned(value) => value.as_tensor(),
        }
    }

    fn to_output(&self) -> InferenceValue {
        match self {
            Self::BorrowedTensor(tensor) => InferenceValue::Tensor((*tensor).clone()),
            Self::Owned(value) => value.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct InferenceNode {
    op: InferenceOp,
    inputs: Vec<InferenceTensorId>,
    output: InferenceTensorId,
}

#[derive(Clone, Debug)]
enum TensorSpec {
    Input(InferenceInput),
    Constant {
        name: String,
        shape: InferenceShape,
        tensor: DenseTensorF32,
    },
    NodeOutput {
        shape: InferenceShape,
    },
}

impl TensorSpec {
    fn shape(&self) -> &InferenceShape {
        match self {
            Self::Input(input) => input.shape(),
            Self::Constant { shape, .. } | Self::NodeOutput { shape, .. } => shape,
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Input(input) => Some(input.name()),
            Self::Constant { name, .. } => Some(name),
            Self::NodeOutput { .. } => None,
        }
    }
}

/// Error for graph validation and forward execution.
#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("inference tensor rank {rank} is invalid")]
    InvalidRank { rank: usize },
    #[error("inference tensor dimension must be non-zero")]
    InvalidZeroDimension,
    #[error("inference tensor shape element count overflowed")]
    ShapeOverflow,
    #[error("unknown inference tensor {id:?}")]
    UnknownTensor { id: InferenceTensorId },
    #[error("inference graph has no outputs")]
    NoOutputs,
    #[error("missing inference input `{name}`")]
    MissingInput { name: String },
    #[error("inference op {op:?} expected matrix tensors, got {shape:?}")]
    ExpectedMatrix {
        op: InferenceOp,
        shape: InferenceShape,
    },
    #[error("inference op {op:?} shape mismatch: lhs {lhs:?}, rhs {rhs:?}")]
    ShapeMismatch {
        op: InferenceOp,
        lhs: InferenceShape,
        rhs: InferenceShape,
    },
    #[error("bias add shape mismatch: tensor {tensor:?}, bias {bias:?}")]
    BiasShapeMismatch {
        tensor: InferenceShape,
        bias: InferenceShape,
    },
    #[error("conv2d shape mismatch: input {input:?}, kernel {kernel:?}")]
    Conv2dShapeMismatch {
        input: InferenceShape,
        kernel: InferenceShape,
    },
    #[error("pooling shape mismatch: input {input:?}, kernel {kernel_y}x{kernel_x}")]
    PoolingShapeMismatch {
        input: InferenceShape,
        kernel_y: usize,
        kernel_x: usize,
    },
    #[error("inference window dimensions must be non-zero")]
    InvalidWindow,
    #[error("inference tensor shape mismatch: expected {expected:?}, found {found:?}")]
    TensorShapeMismatch {
        expected: InferenceShape,
        found: InferenceShape,
    },
    #[error("inference tensor {id:?} was not computed")]
    UncomputedTensor { id: InferenceTensorId },
    #[error("inference value for tensor {id:?} is not a dense tensor")]
    ExpectedTensorValue { id: InferenceTensorId },
    #[error(transparent)]
    Math(#[from] RuntimeMathError),
    #[error(transparent)]
    Accelerator(#[from] RuntimeMathAcceleratorError),
}

fn tensor_value<'a>(
    values: &'a [Option<InferenceRunValue<'a>>],
    id: InferenceTensorId,
) -> Result<&'a DenseTensorF32, InferenceError> {
    values
        .get(id.index())
        .and_then(Option::as_ref)
        .ok_or(InferenceError::UncomputedTensor { id })?
        .as_tensor()
        .ok_or(InferenceError::ExpectedTensorValue { id })
}

fn tensor_as_matrix(
    tensor: &DenseTensorF32,
    op: InferenceOp,
) -> Result<DenseMatrixF32, InferenceError> {
    tensor
        .as_matrix()
        .ok_or_else(|| InferenceError::ExpectedMatrix {
            op,
            shape: InferenceShape::new(tensor.shape().dims().to_vec())
                .expect("runtime tensor shape has non-zero rank"),
        })
}

fn matrix_dims(shape: &InferenceShape, op: InferenceOp) -> Result<[usize; 2], InferenceError> {
    match shape.dims() {
        [rows, cols] => Ok([*rows, *cols]),
        _ => Err(InferenceError::ExpectedMatrix {
            op,
            shape: shape.clone(),
        }),
    }
}

fn nchw_dims(shape: &InferenceShape, op: InferenceOp) -> Result<[usize; 4], InferenceError> {
    match shape.dims() {
        [batch, channels, height, width] => Ok([*batch, *channels, *height, *width]),
        _ => Err(InferenceError::ExpectedMatrix {
            op,
            shape: shape.clone(),
        }),
    }
}

fn oihw_dims(shape: &InferenceShape, op: InferenceOp) -> Result<[usize; 4], InferenceError> {
    match shape.dims() {
        [output_channels, input_channels, height, width] => {
            Ok([*output_channels, *input_channels, *height, *width])
        }
        _ => Err(InferenceError::ExpectedMatrix {
            op,
            shape: shape.clone(),
        }),
    }
}

fn validate_window(first: usize, second: usize) -> Result<(), InferenceError> {
    if first == 0 || second == 0 {
        Err(InferenceError::InvalidWindow)
    } else {
        Ok(())
    }
}

fn validate_tensor_shape(
    tensor: &DenseTensorF32,
    expected: &InferenceShape,
) -> Result<(), InferenceError> {
    let found = InferenceShape::new(tensor.shape().dims().to_vec())?;
    if &found == expected {
        Ok(())
    } else {
        Err(InferenceError::TensorShapeMismatch {
            expected: expected.clone(),
            found,
        })
    }
}

pub(crate) fn bias_add(
    tensor: &DenseTensorF32,
    bias: &DenseTensorF32,
) -> Result<DenseTensorF32, InferenceError> {
    if bias.shape().rank() != 1 {
        return Err(InferenceError::BiasShapeMismatch {
            tensor: InferenceShape::new(tensor.shape().dims().to_vec())?,
            bias: InferenceShape::new(bias.shape().dims().to_vec())?,
        });
    }
    let width = bias.shape().dims()[0];
    if tensor.shape().dims().last().copied() != Some(width) {
        return Err(InferenceError::BiasShapeMismatch {
            tensor: InferenceShape::new(tensor.shape().dims().to_vec())?,
            bias: InferenceShape::new(bias.shape().dims().to_vec())?,
        });
    }
    let values = tensor
        .values()
        .chunks_exact(width)
        .flat_map(|row| {
            row.iter()
                .zip(bias.values())
                .map(|(value, bias)| value + bias)
        })
        .collect();
    DenseTensorF32::new(tensor.shape().dims().to_vec(), values).map_err(Into::into)
}

pub(crate) fn conv2d_valid_nchw(
    input: &DenseTensorF32,
    kernel: &DenseTensorF32,
    stride_y: usize,
    stride_x: usize,
) -> Result<DenseTensorF32, InferenceError> {
    validate_window(stride_y, stride_x)?;
    let input_shape = InferenceShape::new(input.shape().dims().to_vec())?;
    let kernel_shape = InferenceShape::new(kernel.shape().dims().to_vec())?;
    let [batch, input_channels, input_height, input_width] = nchw_dims(
        &input_shape,
        InferenceOp::Conv2dValid { stride_y, stride_x },
    )?;
    let [
        output_channels,
        kernel_channels,
        kernel_height,
        kernel_width,
    ] = oihw_dims(
        &kernel_shape,
        InferenceOp::Conv2dValid { stride_y, stride_x },
    )?;
    if input_channels != kernel_channels
        || input_height < kernel_height
        || input_width < kernel_width
    {
        return Err(InferenceError::Conv2dShapeMismatch {
            input: input_shape,
            kernel: kernel_shape,
        });
    }
    let output_height = (input_height - kernel_height) / stride_y + 1;
    let output_width = (input_width - kernel_width) / stride_x + 1;
    let mut out = vec![0.0; batch * output_channels * output_height * output_width];
    for batch_index in 0..batch {
        for output_channel in 0..output_channels {
            for output_y in 0..output_height {
                for output_x in 0..output_width {
                    let mut sum = 0.0_f32;
                    for input_channel in 0..input_channels {
                        for kernel_y in 0..kernel_height {
                            for kernel_x in 0..kernel_width {
                                let input_y = output_y * stride_y + kernel_y;
                                let input_x = output_x * stride_x + kernel_x;
                                let input_index = nchw_index(
                                    batch_index,
                                    input_channel,
                                    input_y,
                                    input_x,
                                    input_channels,
                                    input_height,
                                    input_width,
                                );
                                let kernel_index = oihw_index(
                                    output_channel,
                                    input_channel,
                                    kernel_y,
                                    kernel_x,
                                    input_channels,
                                    kernel_height,
                                    kernel_width,
                                );
                                sum += input.values()[input_index] * kernel.values()[kernel_index];
                            }
                        }
                    }
                    let output_index = nchw_index(
                        batch_index,
                        output_channel,
                        output_y,
                        output_x,
                        output_channels,
                        output_height,
                        output_width,
                    );
                    out[output_index] = sum;
                }
            }
        }
    }
    DenseTensorF32::new(
        vec![batch, output_channels, output_height, output_width],
        out,
    )
    .map_err(Into::into)
}

pub(crate) fn max_pool2d_nchw(
    input: &DenseTensorF32,
    kernel_y: usize,
    kernel_x: usize,
    stride_y: usize,
    stride_x: usize,
) -> Result<DenseTensorF32, InferenceError> {
    validate_window(kernel_y, kernel_x)?;
    validate_window(stride_y, stride_x)?;
    let input_shape = InferenceShape::new(input.shape().dims().to_vec())?;
    let [batch, channels, input_height, input_width] = nchw_dims(
        &input_shape,
        InferenceOp::MaxPool2d {
            kernel_y,
            kernel_x,
            stride_y,
            stride_x,
        },
    )?;
    if input_height < kernel_y || input_width < kernel_x {
        return Err(InferenceError::PoolingShapeMismatch {
            input: input_shape,
            kernel_y,
            kernel_x,
        });
    }
    let output_height = (input_height - kernel_y) / stride_y + 1;
    let output_width = (input_width - kernel_x) / stride_x + 1;
    let mut out = vec![0.0; batch * channels * output_height * output_width];
    for batch_index in 0..batch {
        for channel in 0..channels {
            for output_y in 0..output_height {
                for output_x in 0..output_width {
                    let mut max = f32::NEG_INFINITY;
                    for window_y in 0..kernel_y {
                        for window_x in 0..kernel_x {
                            let input_y = output_y * stride_y + window_y;
                            let input_x = output_x * stride_x + window_x;
                            let input_index = nchw_index(
                                batch_index,
                                channel,
                                input_y,
                                input_x,
                                channels,
                                input_height,
                                input_width,
                            );
                            max = max.max(input.values()[input_index]);
                        }
                    }
                    let output_index = nchw_index(
                        batch_index,
                        channel,
                        output_y,
                        output_x,
                        channels,
                        output_height,
                        output_width,
                    );
                    out[output_index] = max;
                }
            }
        }
    }
    DenseTensorF32::new(vec![batch, channels, output_height, output_width], out).map_err(Into::into)
}

fn nchw_index(
    batch: usize,
    channel: usize,
    y: usize,
    x: usize,
    channels: usize,
    height: usize,
    width: usize,
) -> usize {
    ((batch * channels + channel) * height + y) * width + x
}

fn oihw_index(
    output_channel: usize,
    input_channel: usize,
    y: usize,
    x: usize,
    input_channels: usize,
    height: usize,
    width: usize,
) -> usize {
    ((output_channel * input_channels + input_channel) * height + y) * width + x
}

pub(crate) fn map_tensor(
    tensor: &DenseTensorF32,
    mut map: impl FnMut(f32) -> f32,
) -> Result<DenseTensorF32, InferenceError> {
    DenseTensorF32::new(
        tensor.shape().dims().to_vec(),
        tensor.values().iter().copied().map(&mut map).collect(),
    )
    .map_err(Into::into)
}

pub(crate) fn softmax_last_dim(tensor: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError> {
    let width = tensor
        .shape()
        .dims()
        .last()
        .copied()
        .expect("runtime tensor rank is non-zero");
    let mut out = Vec::with_capacity(tensor.values().len());
    for row in tensor.values().chunks_exact(width) {
        let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0_f32;
        let start = out.len();
        out.extend(row.iter().map(|value| {
            let exp = (*value - max).exp();
            sum += exp;
            exp
        }));
        for value in &mut out[start..] {
            *value /= sum;
        }
    }
    DenseTensorF32::new(tensor.shape().dims().to_vec(), out).map_err(Into::into)
}

pub(crate) fn argmax_last_dim(tensor: &DenseTensorF32) -> Vec<usize> {
    let width = tensor
        .shape()
        .dims()
        .last()
        .copied()
        .expect("runtime tensor rank is non-zero");
    tensor
        .values()
        .chunks_exact(width)
        .map(|row| {
            row.iter()
                .copied()
                .enumerate()
                .max_by(|lhs, rhs| lhs.1.total_cmp(&rhs.1))
                .map(|(index, _)| index)
                .expect("argmax row is non-empty")
        })
        .collect()
}

pub(crate) fn flatten_outer(tensor: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError> {
    let dims = tensor.shape().dims();
    if dims.len() == 1 {
        return DenseTensorF32::new(vec![1, dims[0]], tensor.values().to_vec()).map_err(Into::into);
    }
    let outer = dims[0];
    let inner = checked_product(&dims[1..])?;
    DenseTensorF32::new(vec![outer, inner], tensor.values().to_vec()).map_err(Into::into)
}

fn checked_product(dims: &[usize]) -> Result<usize, InferenceError> {
    dims.iter().try_fold(1_usize, |acc, dim| {
        acc.checked_mul(*dim).ok_or(InferenceError::ShapeOverflow)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{RuntimeMathAcceleratorConfig, RuntimeMathBackend};

    #[test]
    fn forward_graph_runs_dense_relu_softmax_argmax() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(1, 3).unwrap());
        let weights = builder
            .add_constant(
                "w",
                DenseTensorF32::new(vec![3, 2], vec![1.0, -1.0, 2.0, 0.5, -1.0, 3.0]).unwrap(),
            )
            .unwrap();
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap())
            .unwrap();
        let logits = builder.add_matmul(input, weights).unwrap();
        let logits = builder.add_bias_add(logits, bias).unwrap();
        let logits = builder.add_relu(logits).unwrap();
        let probabilities = builder.add_softmax_last_dim(logits).unwrap();
        let class = builder.add_argmax_last_dim(probabilities).unwrap();
        builder.set_outputs([probabilities, class]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig {
                    backend: RuntimeMathBackend::Scalar,
                    ..RuntimeMathAcceleratorConfig::default()
                },
            )),
        );

        let out = session
            .run([(
                input,
                DenseTensorF32::new(vec![1, 3], vec![1.0, 2.0, 0.5]).unwrap(),
            )])
            .unwrap();

        let probabilities = out[0].as_tensor().unwrap();
        let class = out[1].as_class_indices().unwrap();
        assert_eq!(probabilities.shape().dims(), &[1, 2]);
        assert_eq!(class, &[0]);
        assert!((probabilities.values().iter().sum::<f32>() - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn session_accepts_borrowed_inputs_without_consuming_tensors() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(1, 2).unwrap());
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap())
            .unwrap();
        let output = builder.add_bias_add(input, bias).unwrap();
        builder.set_outputs([output]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig {
                    backend: RuntimeMathBackend::Scalar,
                    ..RuntimeMathAcceleratorConfig::default()
                },
            )),
        );
        let input_tensor = DenseTensorF32::new(vec![1, 2], vec![1.0, 2.0]).unwrap();

        let out = session.run_borrowed([(input, &input_tensor)]).unwrap();

        assert_eq!(out[0].as_tensor().unwrap().values(), &[1.5, 1.75]);
        assert_eq!(input_tensor.values(), &[1.0, 2.0]);
    }

    #[test]
    fn session_fuses_private_matmul_bias_add_at_adapter_boundary() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(1, 3).unwrap());
        let weights = builder
            .add_constant(
                "w",
                DenseTensorF32::new(vec![3, 2], vec![1.0, -1.0, 2.0, 0.5, -1.0, 3.0]).unwrap(),
            )
            .unwrap();
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap())
            .unwrap();
        let logits = builder.add_matmul(input, weights).unwrap();
        let logits = builder.add_bias_add(logits, bias).unwrap();
        builder.set_outputs([logits]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(graph, CountingInferenceAdapter::new());

        let out = session
            .run([(
                input,
                DenseTensorF32::new(vec![1, 3], vec![1.0, 2.0, 0.5]).unwrap(),
            )])
            .unwrap();

        assert_eq!(out[0].as_tensor().unwrap().values(), &[5.0, 1.25]);
        assert_eq!(session.adapter().fused_matmul_bias_add_calls, 1);
        assert_eq!(session.adapter().matmul_calls, 0);
        assert_eq!(session.adapter().bias_add_calls, 0);
        assert_eq!(
            session
                .adapter()
                .inner
                .accelerator()
                .stats()
                .fused_matmul_bias_add_calls,
            1
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn accelerated_session_reuses_prepared_wgpu_matmul_bias_add_buffers() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(16, 16).unwrap());
        let weights = builder
            .add_constant(
                "w",
                DenseTensorF32::new(vec![16, 16], vec![2.0; 256]).unwrap(),
            )
            .unwrap();
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![16], vec![0.25; 16]).unwrap())
            .unwrap();
        let logits = builder.add_matmul(input, weights).unwrap();
        let logits = builder.add_bias_add(logits, bias).unwrap();
        builder.set_outputs([logits]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig {
                    backend: RuntimeMathBackend::Wgpu,
                    ..RuntimeMathAcceleratorConfig::default()
                },
            )),
        );
        let input_value = DenseTensorF32::new(vec![16, 16], vec![1.0; 256]).unwrap();

        let Ok(first) = session.run([(input, input_value.clone())]) else {
            return;
        };
        let first = first[0].as_tensor().unwrap().clone();
        assert_eq!(
            session.adapter().accelerator().stats().gpu_buffer_creations,
            7
        );

        session.adapter_mut().accelerator_mut().reset_stats();
        let second = session
            .run([(input, input_value)])
            .expect("accelerated inference session reuses prepared matmul-bias cache");
        let second = second[0].as_tensor().unwrap();

        assert_eq!(second.values(), first.values());
        assert_eq!(session.adapter().accelerator().stats().wgpu_calls, 1);
        assert_eq!(
            session
                .adapter()
                .accelerator()
                .stats()
                .fused_matmul_bias_add_calls,
            1
        );
        assert_eq!(
            session.adapter().accelerator().stats().gpu_buffer_creations,
            0
        );
        assert_eq!(
            session
                .adapter()
                .accelerator()
                .stats()
                .gpu_buffer_reuse_hits,
            7
        );
        assert_eq!(
            session
                .adapter()
                .accelerator()
                .stats()
                .gpu_reused_dispatches,
            1
        );
        assert_eq!(session.adapter().accelerator().stats().bytes_uploaded, 0);
        assert_eq!(
            session.adapter().accelerator().stats().bytes_downloaded,
            std::mem::size_of_val(second.values())
        );
    }

    #[test]
    fn session_does_not_fuse_when_matmul_output_is_observable() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(1, 2).unwrap());
        let weights = builder
            .add_constant(
                "w",
                DenseTensorF32::new(vec![2, 2], vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
            )
            .unwrap();
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![2], vec![1.0, 2.0]).unwrap())
            .unwrap();
        let product = builder.add_matmul(input, weights).unwrap();
        let logits = builder.add_bias_add(product, bias).unwrap();
        builder.set_outputs([product, logits]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(graph, CountingInferenceAdapter::new());

        let out = session
            .run([(
                input,
                DenseTensorF32::new(vec![1, 2], vec![3.0, 4.0]).unwrap(),
            )])
            .unwrap();

        assert_eq!(out[0].as_tensor().unwrap().values(), &[3.0, 4.0]);
        assert_eq!(out[1].as_tensor().unwrap().values(), &[4.0, 6.0]);
        assert_eq!(session.adapter().fused_matmul_bias_add_calls, 0);
        assert_eq!(session.adapter().matmul_calls, 1);
        assert_eq!(session.adapter().bias_add_calls, 1);
    }

    #[test]
    fn adapter_runs_conv2d_relu_and_max_pool() {
        let mut builder = InferenceGraph::builder();
        let image = builder.add_input("image", InferenceShape::new(vec![1, 1, 4, 4]).unwrap());
        let kernel = builder
            .add_constant(
                "kernel",
                DenseTensorF32::new(vec![1, 1, 2, 2], vec![1.0, 0.0, 0.0, -1.0]).unwrap(),
            )
            .unwrap();
        let conv = builder.add_conv2d_valid(image, kernel, 1, 1).unwrap();
        let relu = builder.add_relu(conv).unwrap();
        let pooled = builder.add_max_pool2d(relu, 2, 2, 1, 1).unwrap();
        builder.set_outputs([pooled]).unwrap();
        let graph = builder.build().unwrap();
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig {
                    backend: RuntimeMathBackend::Scalar,
                    ..RuntimeMathAcceleratorConfig::default()
                },
            )),
        );

        let out = session
            .run([(
                image,
                DenseTensorF32::new(
                    vec![1, 1, 4, 4],
                    vec![
                        8.0, 1.0, 1.0, 1.0, 1.0, 4.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0,
                        1.0,
                    ],
                )
                .unwrap(),
            )])
            .unwrap();

        let pooled = out[0].as_tensor().unwrap();
        assert_eq!(pooled.shape().dims(), &[1, 1, 2, 2]);
        assert_eq!(pooled.values(), &[4.0, 2.0, 2.0, 2.0]);
    }

    #[test]
    fn toy_mnist_cnn_can_classify_a_single_image() {
        let mut builder = InferenceGraph::builder();
        let image = builder.add_input("image", InferenceShape::new(vec![1, 1, 28, 28]).unwrap());
        let kernel = builder
            .add_constant(
                "conv0.weight",
                DenseTensorF32::new(vec![2, 1, 3, 3], toy_mnist_conv_weight()).unwrap(),
            )
            .unwrap();
        let dense_weight = builder
            .add_constant(
                "dense.weight",
                DenseTensorF32::new(vec![338, 10], toy_mnist_cnn_dense_weight()).unwrap(),
            )
            .unwrap();
        let dense_bias = builder
            .add_constant(
                "dense.bias",
                DenseTensorF32::new(vec![10], vec![0.0; 10]).unwrap(),
            )
            .unwrap();
        let conv = builder.add_conv2d_valid(image, kernel, 1, 1).unwrap();
        let conv = builder.add_relu(conv).unwrap();
        let pooled = builder.add_max_pool2d(conv, 2, 2, 2, 2).unwrap();
        let flat = builder.add_flatten_outer(pooled).unwrap();
        let logits = builder.add_matmul(flat, dense_weight).unwrap();
        let logits = builder.add_bias_add(logits, dense_bias).unwrap();
        let class = builder.add_argmax_last_dim(logits).unwrap();
        builder.set_outputs([class]).unwrap();
        let graph = builder.build().unwrap();
        let mut pixels = vec![0.0; 28 * 28];
        pixels[13 * 28 + 14] = 1.0;
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig {
                    backend: RuntimeMathBackend::Scalar,
                    ..RuntimeMathAcceleratorConfig::default()
                },
            )),
        );

        let out = session
            .run([(
                image,
                DenseTensorF32::new(vec![1, 1, 28, 28], pixels).unwrap(),
            )])
            .unwrap();

        assert_eq!(out[0].as_class_indices().unwrap(), &[7]);
    }

    #[test]
    fn toy_mnist_mlp_can_classify_a_single_image() {
        let mut builder = InferenceGraph::builder();
        let image = builder.add_input("image", InferenceShape::new(vec![1, 28, 28]).unwrap());
        let flat = builder.add_flatten_outer(image).unwrap();
        let first_weight = builder
            .add_constant(
                "dense0.weight",
                DenseTensorF32::new(vec![784, 16], toy_mnist_first_weight()).unwrap(),
            )
            .unwrap();
        let first_bias = builder
            .add_constant(
                "dense0.bias",
                DenseTensorF32::new(vec![16], vec![0.0; 16]).unwrap(),
            )
            .unwrap();
        let second_weight = builder
            .add_constant(
                "dense1.weight",
                DenseTensorF32::new(vec![16, 10], toy_mnist_second_weight()).unwrap(),
            )
            .unwrap();
        let second_bias = builder
            .add_constant(
                "dense1.bias",
                DenseTensorF32::new(vec![10], vec![0.0; 10]).unwrap(),
            )
            .unwrap();
        let hidden = builder.add_matmul(flat, first_weight).unwrap();
        let hidden = builder.add_bias_add(hidden, first_bias).unwrap();
        let hidden = builder.add_relu(hidden).unwrap();
        let logits = builder.add_matmul(hidden, second_weight).unwrap();
        let logits = builder.add_bias_add(logits, second_bias).unwrap();
        let class = builder.add_argmax_last_dim(logits).unwrap();
        builder.set_outputs([class]).unwrap();
        let graph = builder.build().unwrap();
        let mut pixels = vec![0.0; 28 * 28];
        pixels[13 * 28 + 14] = 1.0;
        let mut session = InferenceSession::new(
            graph,
            AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                RuntimeMathAcceleratorConfig::default(),
            )),
        );

        let out = session
            .run([(image, DenseTensorF32::new(vec![1, 28, 28], pixels).unwrap())])
            .unwrap();

        assert_eq!(out[0].as_class_indices().unwrap(), &[7]);
    }

    #[test]
    fn builder_rejects_bad_bias_shape() {
        let mut builder = InferenceGraph::builder();
        let input = builder.add_input("x", InferenceShape::matrix(1, 4).unwrap());
        let bias = builder
            .add_constant("b", DenseTensorF32::new(vec![3], vec![0.0; 3]).unwrap())
            .unwrap();

        let error = builder.add_bias_add(input, bias).unwrap_err();

        assert!(matches!(error, InferenceError::BiasShapeMismatch { .. }));
    }

    struct CountingInferenceAdapter {
        inner: AcceleratedInferenceAdapter,
        matmul_calls: usize,
        bias_add_calls: usize,
        fused_matmul_bias_add_calls: usize,
    }

    impl CountingInferenceAdapter {
        fn new() -> Self {
            Self {
                inner: AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
                    RuntimeMathAcceleratorConfig {
                        backend: RuntimeMathBackend::Scalar,
                        ..RuntimeMathAcceleratorConfig::default()
                    },
                )),
                matmul_calls: 0,
                bias_add_calls: 0,
                fused_matmul_bias_add_calls: 0,
            }
        }
    }

    impl InferenceAdapter for CountingInferenceAdapter {
        fn matmul(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.matmul_calls += 1;
            self.inner.matmul(lhs, rhs)
        }

        fn add(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.inner.add(lhs, rhs)
        }

        fn bias_add(
            &mut self,
            tensor: &DenseTensorF32,
            bias: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.bias_add_calls += 1;
            self.inner.bias_add(tensor, bias)
        }

        fn matmul_bias_add(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
            bias: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.fused_matmul_bias_add_calls += 1;
            self.inner.matmul_bias_add(lhs, rhs, bias)
        }

        fn conv2d_valid(
            &mut self,
            input: &DenseTensorF32,
            kernel: &DenseTensorF32,
            stride_y: usize,
            stride_x: usize,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.inner.conv2d_valid(input, kernel, stride_y, stride_x)
        }

        fn relu(&mut self, input: &DenseTensorF32) -> Result<DenseTensorF32, InferenceError> {
            self.inner.relu(input)
        }

        fn max_pool2d(
            &mut self,
            input: &DenseTensorF32,
            kernel_y: usize,
            kernel_x: usize,
            stride_y: usize,
            stride_x: usize,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.inner
                .max_pool2d(input, kernel_y, kernel_x, stride_y, stride_x)
        }

        fn softmax_last_dim(
            &mut self,
            input: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.inner.softmax_last_dim(input)
        }

        fn argmax_last_dim(&mut self, input: &DenseTensorF32) -> Vec<usize> {
            self.inner.argmax_last_dim(input)
        }

        fn flatten_outer(
            &mut self,
            input: &DenseTensorF32,
        ) -> Result<DenseTensorF32, InferenceError> {
            self.inner.flatten_outer(input)
        }
    }

    fn toy_mnist_conv_weight() -> Vec<f32> {
        let mut values = vec![0.0; 2 * 3 * 3];
        values[4] = 1.0;
        values[9 + 4] = -1.0;
        values
    }

    fn toy_mnist_cnn_dense_weight() -> Vec<f32> {
        let mut values = vec![0.0; 338 * 10];
        let feature_index = 6 * 13 + 6;
        values[feature_index * 10 + 7] = 3.0;
        values
    }

    fn toy_mnist_first_weight() -> Vec<f32> {
        let mut values = vec![0.0; 784 * 16];
        values[(13 * 28 + 14) * 16 + 3] = 2.0;
        values
    }

    fn toy_mnist_second_weight() -> Vec<f32> {
        let mut values = vec![0.0; 16 * 10];
        values[3 * 10 + 7] = 3.0;
        values
    }
}
