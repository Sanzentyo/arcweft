use super::*;

impl BrowserWebGpuAutoMathAdapter {
    pub async fn new(policy: BrowserWebGpuMathAutoPolicy) -> Result<Self, BrowserWebGpuError> {
        let context = BrowserWebGpuMathContext::new().await?;
        Ok(Self::from_context(context, policy))
    }

    pub const fn from_context(
        context: BrowserWebGpuMathContext,
        policy: BrowserWebGpuMathAutoPolicy,
    ) -> Self {
        Self {
            context,
            policy,
            elementwise_f32: None,
            matmul_f32: None,
        }
    }

    pub const fn policy(&self) -> BrowserWebGpuMathAutoPolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: BrowserWebGpuMathAutoPolicy) {
        self.policy = policy;
    }

    pub const fn limits(&self) -> BrowserWebGpuLimits {
        self.context.limits()
    }

    pub const fn stats(&self) -> BrowserWebGpuMathStats {
        self.context.stats()
    }

    pub fn reset_stats(&mut self) {
        self.context.reset_stats();
    }

    pub fn clear_prepared_buffers(&mut self) {
        self.elementwise_f32 = None;
        self.matmul_f32 = None;
    }

    pub const fn context(&self) -> &BrowserWebGpuMathContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut BrowserWebGpuMathContext {
        &mut self.context
    }

    pub async fn dispatch(
        &mut self,
        request: BrowserWebGpuMathRequest<'_>,
    ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
        let dispatch = self.submit(request)?;
        self.read_dispatch(dispatch).await
    }

    pub fn submit(
        &mut self,
        request: BrowserWebGpuMathRequest<'_>,
    ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
        match request {
            BrowserWebGpuMathRequest::MatmulF32 { lhs, rhs } => self.submit_matmul_f32(lhs, rhs),
            BrowserWebGpuMathRequest::MatrixAddF32 { lhs, rhs } => {
                self.submit_matrix_add_f32(lhs, rhs)
            }
            BrowserWebGpuMathRequest::TensorAddF32 { lhs, rhs } => {
                self.submit_tensor_add_f32(lhs, rhs)
            }
        }
    }

    pub fn prepare_resident(
        &mut self,
        request: BrowserWebGpuMathRequest<'_>,
    ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
        match request {
            BrowserWebGpuMathRequest::MatmulF32 { lhs, rhs } => {
                self.prepare_resident_matmul_f32(lhs, rhs)
            }
            BrowserWebGpuMathRequest::MatrixAddF32 { lhs, rhs } => {
                self.prepare_resident_matrix_add_f32(lhs, rhs)
            }
            BrowserWebGpuMathRequest::TensorAddF32 { lhs, rhs } => {
                self.prepare_resident_tensor_add_f32(lhs, rhs)
            }
        }
    }

    pub fn submit_prepared(
        &mut self,
        prepared: &BrowserWebGpuPreparedMath,
    ) -> Result<BrowserWebGpuSubmittedMath, BrowserWebGpuError> {
        match prepared {
            BrowserWebGpuPreparedMath::MatmulF32 {
                prepared,
                rows,
                cols,
                selection,
            } => {
                let submitted = self
                    .context
                    .submit_resident_matmul_f32(prepared, *rows, *cols)?;
                Ok(BrowserWebGpuSubmittedMath::MatrixF32 {
                    submitted,
                    rows: *rows,
                    cols: *cols,
                    selection: *selection,
                })
            }
            BrowserWebGpuPreparedMath::MatrixAddF32 {
                prepared,
                rows,
                cols,
                len,
                selection,
            } => {
                let submitted = self
                    .context
                    .submit_resident_elementwise_f32(prepared, *len)?;
                Ok(BrowserWebGpuSubmittedMath::MatrixF32 {
                    submitted,
                    rows: *rows,
                    cols: *cols,
                    selection: *selection,
                })
            }
            BrowserWebGpuPreparedMath::TensorAddF32 {
                prepared,
                dims,
                len,
                selection,
            } => {
                let submitted = self
                    .context
                    .submit_resident_elementwise_f32(prepared, *len)?;
                Ok(BrowserWebGpuSubmittedMath::TensorF32 {
                    submitted,
                    dims: dims.clone(),
                    selection: *selection,
                })
            }
        }
    }

    pub async fn read_dispatch(
        &mut self,
        dispatch: BrowserWebGpuMathDispatch,
    ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
        match dispatch {
            BrowserWebGpuMathDispatch::Immediate(response) => Ok(response),
            BrowserWebGpuMathDispatch::Submitted(submitted) => self.read_submitted(submitted).await,
        }
    }

    pub async fn read_submitted(
        &mut self,
        submitted: BrowserWebGpuSubmittedMath,
    ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
        let mut out = vec![0.0; submitted.len()];
        match self.read_submitted_values_into(submitted, &mut out).await? {
            BrowserWebGpuSubmittedOutput::MatrixF32 {
                rows,
                cols,
                selection,
            } => {
                let value = DenseMatrixF32::new(rows, cols, out)?;
                Ok(BrowserWebGpuMathResponse::MatrixF32(
                    BrowserWebGpuAutoMathResult {
                        value,
                        selection,
                        stats: self.context.stats,
                    },
                ))
            }
            BrowserWebGpuSubmittedOutput::TensorF32 { dims, selection } => {
                let value = DenseTensorF32::new(dims, out)?;
                Ok(BrowserWebGpuMathResponse::TensorF32(
                    BrowserWebGpuAutoMathResult {
                        value,
                        selection,
                        stats: self.context.stats,
                    },
                ))
            }
        }
    }

    pub async fn read_submitted_values_into(
        &mut self,
        submitted: BrowserWebGpuSubmittedMath,
        out: &mut [f32],
    ) -> Result<BrowserWebGpuSubmittedOutput, BrowserWebGpuError> {
        match submitted {
            BrowserWebGpuSubmittedMath::MatrixF32 {
                submitted,
                rows,
                cols,
                selection,
            } => {
                self.context.read_submitted_f32(submitted, out).await?;
                Ok(BrowserWebGpuSubmittedOutput::MatrixF32 {
                    rows,
                    cols,
                    selection,
                })
            }
            BrowserWebGpuSubmittedMath::TensorF32 {
                submitted,
                dims,
                selection,
            } => {
                self.context.read_submitted_f32(submitted, out).await?;
                Ok(BrowserWebGpuSubmittedOutput::TensorF32 { dims, selection })
            }
        }
    }

    fn submit_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        let selection =
            self.policy
                .select_matmul_f32(lhs.rows(), lhs.cols(), rhs.cols(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                BrowserWebGpuMathResponse::MatrixF32(BrowserWebGpuAutoMathResult {
                    value: lhs.matmul_scalar(rhs)?,
                    selection,
                    stats: self.context.stats,
                }),
            )),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                    rows: lhs.rows(),
                    shared: lhs.cols(),
                    cols: rhs.cols(),
                });
                let prepared = self.take_matmul_f32(capacity)?;
                let submitted = match self.context.upload_prepared_matmul_f32(
                    &prepared,
                    lhs.values(),
                    rhs.values(),
                    lhs.rows(),
                    lhs.cols(),
                    rhs.cols(),
                ) {
                    Ok(()) => {
                        self.context
                            .submit_resident_matmul_f32(&prepared, lhs.rows(), rhs.cols())
                    }
                    Err(error) => Err(error),
                };
                self.matmul_f32 = Some(prepared);
                let submitted = submitted?;
                Ok(BrowserWebGpuMathDispatch::Submitted(
                    BrowserWebGpuSubmittedMath::MatrixF32 {
                        submitted,
                        rows: lhs.rows(),
                        cols: rhs.cols(),
                        selection,
                    },
                ))
            }
        }
    }

    fn prepare_resident_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        let selection =
            self.policy
                .select_matmul_f32(lhs.rows(), lhs.cols(), rhs.cols(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection)),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                    rows: lhs.rows(),
                    shared: lhs.cols(),
                    cols: rhs.cols(),
                });
                let prepared = self.context.prepare_matmul_f32(capacity)?;
                self.context.upload_prepared_matmul_f32(
                    &prepared,
                    lhs.values(),
                    rhs.values(),
                    lhs.rows(),
                    lhs.cols(),
                    rhs.cols(),
                )?;
                Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                    BrowserWebGpuPreparedMath::MatmulF32 {
                        prepared,
                        rows: lhs.rows(),
                        cols: rhs.cols(),
                        selection,
                    },
                ))
            }
        }
    }

    fn submit_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        let selection = self
            .policy
            .select_elementwise_f32(lhs.values().len(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                BrowserWebGpuMathResponse::MatrixF32(BrowserWebGpuAutoMathResult {
                    value: lhs.add_scalar(rhs)?,
                    selection,
                    stats: self.context.stats,
                }),
            )),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let submitted = self.submit_elementwise_f32(lhs.values(), rhs.values())?;
                Ok(BrowserWebGpuMathDispatch::Submitted(
                    BrowserWebGpuSubmittedMath::MatrixF32 {
                        submitted,
                        rows: lhs.rows(),
                        cols: lhs.cols(),
                        selection,
                    },
                ))
            }
        }
    }

    fn prepare_resident_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        let selection = self
            .policy
            .select_elementwise_f32(lhs.values().len(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection)),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let prepared = self.context.prepare_elementwise_f32(lhs.values().len())?;
                self.context.upload_prepared_elementwise_f32(
                    &prepared,
                    lhs.values(),
                    rhs.values(),
                )?;
                Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                    BrowserWebGpuPreparedMath::MatrixAddF32 {
                        prepared,
                        rows: lhs.rows(),
                        cols: lhs.cols(),
                        len: lhs.values().len(),
                        selection,
                    },
                ))
            }
        }
    }

    fn submit_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        let selection = self
            .policy
            .select_elementwise_f32(lhs.values().len(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                BrowserWebGpuMathResponse::TensorF32(BrowserWebGpuAutoMathResult {
                    value: lhs.add_scalar(rhs)?,
                    selection,
                    stats: self.context.stats,
                }),
            )),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let submitted = self.submit_elementwise_f32(lhs.values(), rhs.values())?;
                Ok(BrowserWebGpuMathDispatch::Submitted(
                    BrowserWebGpuSubmittedMath::TensorF32 {
                        submitted,
                        dims: lhs.shape().dims().to_vec(),
                        selection,
                    },
                ))
            }
        }
    }

    fn prepare_resident_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        let selection = self
            .policy
            .select_elementwise_f32(lhs.values().len(), self.context.limits);
        match selection.mode() {
            BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection)),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
            | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                let prepared = self.context.prepare_elementwise_f32(lhs.values().len())?;
                self.context.upload_prepared_elementwise_f32(
                    &prepared,
                    lhs.values(),
                    rhs.values(),
                )?;
                Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                    BrowserWebGpuPreparedMath::TensorAddF32 {
                        prepared,
                        dims: lhs.shape().dims().to_vec(),
                        len: lhs.values().len(),
                        selection,
                    },
                ))
            }
        }
    }

    fn submit_elementwise_f32(
        &mut self,
        lhs: &[f32],
        rhs: &[f32],
    ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
        let prepared = self.take_elementwise_f32(lhs.len())?;
        let submitted = match self
            .context
            .upload_prepared_elementwise_f32(&prepared, lhs, rhs)
        {
            Ok(()) => self
                .context
                .submit_resident_elementwise_f32(&prepared, lhs.len()),
            Err(error) => Err(error),
        };
        self.elementwise_f32 = Some(prepared);
        submitted
    }

    fn take_elementwise_f32(
        &mut self,
        len: usize,
    ) -> Result<BrowserPreparedElementwiseF32, BrowserWebGpuError> {
        if let Some(prepared) = self.elementwise_f32.take()
            && prepared.capacity_len() >= len
        {
            return Ok(prepared);
        }
        self.context.prepare_elementwise_f32(len)
    }

    fn take_matmul_f32(
        &mut self,
        capacity: BrowserMatmulCapacity,
    ) -> Result<BrowserPreparedMatmulF32, BrowserWebGpuError> {
        if let Some(prepared) = self.matmul_f32.take()
            && matmul_capacity_covers(prepared.capacity(), capacity)
        {
            return Ok(prepared);
        }
        self.context.prepare_matmul_f32(capacity)
    }
}
