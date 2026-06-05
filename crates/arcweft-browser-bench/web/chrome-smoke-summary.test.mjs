import assert from "node:assert/strict";
import { test } from "node:test";

import { summarize } from "./chrome-smoke.mjs";

test("smoke summary preserves dispersion fields for measured cases and speedups", () => {
  const report = {
    schema_version: 1,
    run: {
      webgpu: {
        available: true,
        fallback_reason: null,
      },
    },
    cases: [
      measuredCase("cpu", "cpu_wasm", 4.0, 0.4, 4.9),
      measuredCase("gpu", "web_gpu_prepared_resident_pipelined", 1.0, 0.1, 1.2),
    ],
    stability: [
      stabilityCase("cpu_wasm", 2, 4.0, 3.8, 4.2, 0.2),
      stabilityCase("web_gpu_prepared_resident_pipelined", 2, 1.0, 0.9, 1.1, 0.05),
    ],
    recommendations: [
      {
        op: "matrix_matmul",
        shape: {
          matmul: {
            rows: 16,
            shared: 16,
            cols: 16,
          },
        },
        selected_mode: "web_gpu_prepared_resident_pipelined",
        selected_median_ms: 1.0,
        selected_mad_ms: 0.1,
        selected_p95_ms: 1.2,
        cpu_median_ms: 4.0,
        cpu_mad_ms: 0.4,
        cpu_p95_ms: 4.9,
        speedup: 4.0,
        reason: "web_gpu_faster",
        selected_capacity: null,
      },
    ],
  };

  const summary = summarize(report);

  assert.equal(summary.failed_cases, 0);
  assert.deepEqual(summary.representative[0], {
    case_id: "cpu",
    mode: "cpu_wasm",
    median_ms: 4.0,
    mad_ms: 0.4,
    min_ms: 3.6,
    p95_ms: 4.9,
    effective_gflops: 0.25,
    submit_ms: null,
    readback_ms: null,
    submit_share: null,
    readback_share: null,
    correct: true,
  });
  assert.deepEqual(summary.best_speedups[0], {
    case: "matrix_matmul_m16_k16_n16",
    mode: "web_gpu_prepared_resident_pipelined",
    cpu_ms: 4.0,
    gpu_ms: 1.0,
    gpu_min_ms: 0.9,
    gpu_max_ms: 1.1,
    gpu_mad_ms: 0.05,
    speedup: 4.0,
    effective_gflops: 0.004096,
    submit_ms: null,
    readback_ms: null,
    submit_share: null,
    readback_share: null,
    workgroups: 1,
    estimated_flops: 4096,
    rounds: 2,
    spread_ratio: 1.1 / 0.9,
  });
  assert.deepEqual(summary.selected_modes[0], {
    op: "matrix_matmul",
    shape: {
      matmul: {
        rows: 16,
        shared: 16,
        cols: 16,
      },
    },
    selected_mode: "web_gpu_prepared_resident_pipelined",
    selected_median_ms: 1.0,
    selected_mad_ms: 0.1,
    selected_p95_ms: 1.2,
    cpu_median_ms: 4.0,
    cpu_mad_ms: 0.4,
    cpu_p95_ms: 4.9,
    speedup: 4.0,
    reason: "web_gpu_faster",
    capacity: null,
  });
});

function measuredCase(caseId, mode, medianMs, madMs, p95Ms) {
  return {
    case_id: caseId,
    op: "matrix_matmul",
    shape: {
      matmul: {
        rows: 16,
        shared: 16,
        cols: 16,
      },
    },
    mode,
    median_ms: medianMs,
    mad_ms: madMs,
    min_ms: medianMs - madMs,
    p95_ms: p95Ms,
    effective_gflops: 0.25,
    estimated_flops: 4096,
    submit_median_ms: null,
    readback_median_ms: null,
    submit_median_share: null,
    readback_median_share: null,
    workgroups: 1,
    fallback_reason: null,
    correctness: {
      passed: true,
    },
  };
}

function stabilityCase(mode, rounds, medianMs, minMs, maxMs, madMs) {
  return {
    op: "matrix_matmul",
    shape: {
      matmul: {
        rows: 16,
        shared: 16,
        cols: 16,
      },
    },
    mode,
    measured_rounds: rounds,
    median_of_medians_ms: medianMs,
    min_median_ms: minMs,
    max_median_ms: maxMs,
    median_mad_ms: madMs,
    spread_ratio: maxMs / minMs,
  };
}
