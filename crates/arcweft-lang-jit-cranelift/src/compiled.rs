use super::{
    CompiledPureF32Inputs, CompiledPureF64Inputs, CompiledPureI8Inputs, CompiledPureI16Inputs,
    CompiledPureI32Inputs, CompiledPureI64, CompiledPureI64Batch, CompiledPureI64Inputs,
    CompiledPureI128BatchInputs, CompiledPureU8Inputs, CompiledPureU16Inputs,
    CompiledPureU32Inputs, CompiledPureU64Inputs, CompiledPureU128BatchInputs,
    CraneliftCodegenError, PureFunctionStats, RuntimeI64Args, RuntimeISizeValue,
    RuntimeLocalDeclarationId, RuntimeUSizeValue, native_call,
};

impl CompiledPureI64 {
    /// Calls the compiled helper.
    pub fn call(&self) -> i64 {
        native_call::call_i64(self.code)
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI64Inputs {
    /// Calls the compiled helper with runtime integer inputs.
    pub fn call(&self, inputs: &[i64]) -> Result<i64, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper with the runtime fixed-size integer pack.
    pub fn call_i64_args(&self, args: RuntimeI64Args) -> Result<i64, CraneliftCodegenError> {
        if args.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT helper expected {} input(s), got {}",
                self.input_locals.len(),
                args.len()
            )));
        }
        let (values, len) = args.into_parts();
        self.caller.call_packed(values, len).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT helper arity {len} is outside the native call boundary"
            ))
        })
    }

    /// Calls the compiled helper with runtime `isize`-semantic inputs.
    pub fn call_isize(
        &self,
        inputs: &[RuntimeISizeValue],
    ) -> Result<RuntimeISizeValue, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT isize helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call_isize(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT isize helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `i64` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[i64],
        out: &mut [i64],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_i64_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `isize`-semantic inputs.
    pub fn call_isize_flat_batch(
        &self,
        inputs: &[RuntimeISizeValue],
        out: &mut [RuntimeISizeValue],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_isize_rows_batch(
            self.batch_code,
            inputs,
            self.input_locals.len(),
            out,
        ) {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT isize rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `i64` inputs and returns
    /// the sum of all row results without writing an output slice.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[i64],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_i64_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT rows batch sum expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len(),
                rows
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `isize` inputs and returns
    /// the sum of all row results without writing an output slice.
    pub fn call_isize_flat_batch_sum(
        &self,
        inputs: &[RuntimeISizeValue],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_isize_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT isize rows batch sum expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len(),
                rows
            ))
        })
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn input_locals(&self) -> &[RuntimeLocalDeclarationId] {
        &self.input_locals
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI128BatchInputs {
    /// Calls the compiled helper for one row of `i128` inputs.
    ///
    /// This intentionally uses the pointer-based row batch ABI instead of a
    /// by-value `i128` native function signature.
    pub fn call(&self, inputs: &[i128]) -> Result<i128, CraneliftCodegenError> {
        let mut out = [0_i128];
        self.call_flat_batch(inputs, &mut out)?;
        Ok(out[0])
    }

    /// Calls the compiled helper for flat row-major `i128` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[i128],
        out: &mut [i128],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_i128_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i128 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `i128` inputs and narrows
    /// each row result into the `sum()` i64 accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[i128],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_i128_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i128 rows batch sum expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len(),
                rows
            ))
        })
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn input_locals(&self) -> &[RuntimeLocalDeclarationId] {
        &self.input_locals
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI32Inputs {
    /// Calls the compiled helper with runtime `i32` inputs.
    pub fn call(&self, inputs: &[i32]) -> Result<i32, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i32 helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `i32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[i32],
        out: &mut [i32],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_i32_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `i32` inputs and sums the
    /// `i32` outputs into an `i64` accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[i32],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_i32_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT i32 rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

macro_rules! impl_compiled_small_int_inputs {
    ($compiled:ty, $ty:ty, $call_batch:path, $call_sum:path, $label:literal) => {
        impl $compiled {
            pub fn call(&self, inputs: &[$ty]) -> Result<$ty, CraneliftCodegenError> {
                if inputs.len() != self.input_locals.len() {
                    return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                        "JIT {} helper expected {} input(s), got {}",
                        $label,
                        self.input_locals.len(),
                        inputs.len()
                    )));
                }
                self.caller.call(inputs).ok_or_else(|| {
                    CraneliftCodegenError::UnsupportedExpr(format!(
                        "JIT {} helper arity {} is outside the native call boundary",
                        $label,
                        inputs.len()
                    ))
                })
            }

            pub fn call_flat_batch(
                &self,
                inputs: &[$ty],
                out: &mut [$ty],
            ) -> Result<(), CraneliftCodegenError> {
                if !$call_batch(self.batch_code, inputs, self.input_locals.len(), out) {
                    return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                        "JIT {} rows batch expected {} input value(s), got {} for {} row(s)",
                        $label,
                        self.input_locals.len().saturating_mul(out.len()),
                        inputs.len(),
                        out.len()
                    )));
                }
                Ok(())
            }

            pub fn call_flat_batch_sum(
                &self,
                inputs: &[$ty],
                rows: usize,
            ) -> Result<i64, CraneliftCodegenError> {
                $call_sum(self.batch_sum_code, inputs, self.input_locals.len(), rows).ok_or_else(
                    || {
                        CraneliftCodegenError::UnsupportedExpr(format!(
                            "JIT {} rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                            $label,
                            self.input_locals.len().saturating_mul(rows),
                            inputs.len()
                        ))
                    },
                )
            }

            pub const fn stats(&self) -> &PureFunctionStats {
                &self.stats
            }
        }
    };
}

impl_compiled_small_int_inputs!(
    CompiledPureI8Inputs,
    i8,
    native_call::call_i8_rows_batch,
    native_call::call_i8_rows_batch_sum,
    "i8"
);
impl_compiled_small_int_inputs!(
    CompiledPureI16Inputs,
    i16,
    native_call::call_i16_rows_batch,
    native_call::call_i16_rows_batch_sum,
    "i16"
);
impl_compiled_small_int_inputs!(
    CompiledPureU8Inputs,
    u8,
    native_call::call_u8_rows_batch,
    native_call::call_u8_rows_batch_sum,
    "u8"
);
impl_compiled_small_int_inputs!(
    CompiledPureU16Inputs,
    u16,
    native_call::call_u16_rows_batch,
    native_call::call_u16_rows_batch_sum,
    "u16"
);

impl CompiledPureU32Inputs {
    /// Calls the compiled helper with runtime `u32` inputs.
    pub fn call(&self, inputs: &[u32]) -> Result<u32, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u32 helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `u32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[u32],
        out: &mut [u32],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_u32_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `u32` inputs and sums the
    /// `u32` outputs into an `i64` accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[u32],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_u32_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u32 rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureU64Inputs {
    /// Calls the compiled helper with runtime `u64` inputs.
    pub fn call(&self, inputs: &[u64]) -> Result<u64, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u64 helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u64 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper with runtime `usize`-semantic inputs.
    pub fn call_usize(
        &self,
        inputs: &[RuntimeUSizeValue],
    ) -> Result<RuntimeUSizeValue, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT usize helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call_usize(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT usize helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `u64` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[u64],
        out: &mut [u64],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_u64_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u64 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `usize`-semantic inputs.
    pub fn call_usize_flat_batch(
        &self,
        inputs: &[RuntimeUSizeValue],
        out: &mut [RuntimeUSizeValue],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_usize_rows_batch(
            self.batch_code,
            inputs,
            self.input_locals.len(),
            out,
        ) {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT usize rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `u64` inputs and sums the
    /// `u64` outputs into the runtime's `i64` reduction accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[u64],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_u64_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u64 rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `usize` inputs and sums the
    /// outputs into the runtime's `i64` reduction accumulator.
    pub fn call_usize_flat_batch_sum(
        &self,
        inputs: &[RuntimeUSizeValue],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_usize_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT usize rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureU128BatchInputs {
    /// Calls the compiled helper for one row of `u128` inputs.
    ///
    /// This intentionally uses the pointer-based row batch ABI instead of a
    /// by-value `u128` native function signature.
    pub fn call(&self, inputs: &[u128]) -> Result<u128, CraneliftCodegenError> {
        let mut out = [0_u128];
        self.call_flat_batch(inputs, &mut out)?;
        Ok(out[0])
    }

    /// Calls the compiled helper for flat row-major `u128` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[u128],
        out: &mut [u128],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_u128_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u128 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `u128` inputs and narrows
    /// each row result into the `sum()` i64 accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[u128],
        rows: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        native_call::call_u128_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.input_locals.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT u128 rows batch sum expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(rows),
                inputs.len(),
                rows
            ))
        })
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn input_locals(&self) -> &[RuntimeLocalDeclarationId] {
        &self.input_locals
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureF32Inputs {
    /// Calls the compiled helper with runtime `f32` inputs.
    pub fn call(&self, inputs: &[f32]) -> Result<f32, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f32 helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `f32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[f32],
        out: &mut [f32],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_f32_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureF64Inputs {
    /// Calls the compiled helper with runtime `f64` inputs.
    pub fn call(&self, inputs: &[f64]) -> Result<f64, CraneliftCodegenError> {
        if inputs.len() != self.input_locals.len() {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f64 helper expected {} input(s), got {}",
                self.input_locals.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f64 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `f64` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[f64],
        out: &mut [f64],
    ) -> Result<(), CraneliftCodegenError> {
        if !native_call::call_f64_rows_batch(self.batch_code, inputs, self.input_locals.len(), out)
        {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT f64 rows batch expected {} input value(s), got {} for {} row(s)",
                self.input_locals.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI64Batch {
    /// Calls the compiled batch helper for a deterministic input series.
    pub fn call(
        &self,
        seed: u64,
        sample: usize,
        iterations: usize,
    ) -> Result<i64, CraneliftCodegenError> {
        let seed = i64::try_from(seed).map_err(|_| {
            CraneliftCodegenError::UnsupportedExpr("JIT batch seed must fit i64".to_owned())
        })?;
        let sample = i64::try_from(sample).map_err(|_| {
            CraneliftCodegenError::UnsupportedExpr("JIT batch sample index must fit i64".to_owned())
        })?;
        let iterations = i64::try_from(iterations).map_err(|_| {
            CraneliftCodegenError::UnsupportedExpr("JIT batch iterations must fit i64".to_owned())
        })?;
        Ok(native_call::call_i64_batch(
            self.code, seed, sample, iterations,
        ))
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn input_locals(&self) -> &[RuntimeLocalDeclarationId] {
        &self.input_locals
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}
