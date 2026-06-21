use crate::model::*;
use crate::recommend::browser_math_bench_recommendations;
use crate::stability::browser_math_bench_stability;

#[test]
fn default_report_schema_serializes_without_paths() {
    let cases = vec![bench_case(
        "tensor_add_f32_len256_capacity",
        "tensor_add_f32",
        BrowserMathBenchShape::Len { len: 256 },
        Some(BrowserMathBenchCapacity::Len { len: 512 }),
        BrowserBenchMode::WebGpuPreparedCapacityResident,
        Some(0.25),
        true,
    )];
    let recommendations = browser_math_bench_recommendations(&cases, None);
    let report = BrowserMathBenchReport {
        schema_version: "arcweft.browser_webgpu_bench.v1",
        run: BrowserMathBenchRun {
            secure_context: true,
            cross_origin_isolated: false,
            webgpu: BrowserMathBenchWebGpu {
                available: false,
                fallback_reason: Some("navigator_gpu_missing".to_owned()),
                limits: None,
            },
        },
        stability: browser_math_bench_stability(&cases),
        cases,
        recommendations,
        skips: vec![BrowserMathBenchSkip {
            scope: "webgpu",
            reason: "navigator_gpu_missing".to_owned(),
        }],
    };

    let json = serde_json::to_string(&report).expect("report serializes");

    assert!(json.contains("arcweft.browser_webgpu_bench.v1"));
    assert!(json.contains("\"capacity\""));
    assert!(json.contains("\"len\":512"));
    assert!(json.contains("\"effective_gflops\""));
    assert!(json.contains("\"submit_median_share\""));
    assert!(json.contains("\"readback_median_share\""));
    assert!(json.contains("\"recommendations\""));
    assert!(json.contains("\"stability\""));
    assert!(json.contains("\"round_index\""));
    assert!(json.contains("\"mode_order_index\""));
    assert!(!json.contains("\\\\"));
    assert!(!json.contains("D:"));
}

#[test]
fn stability_groups_repeated_round_medians() {
    let shape = BrowserMathBenchShape::Matmul {
        rows: 256,
        shared: 256,
        cols: 256,
    };
    let cases = vec![
        bench_case(
            "matmul_round0",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(1.0),
            true,
        ),
        bench_case(
            "matmul_round1",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(1.5),
            true,
        ),
        bench_case(
            "matmul_wrong",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(0.5),
            false,
        ),
    ];

    let stability = browser_math_bench_stability(&cases);

    assert_eq!(stability.len(), 1);
    assert_eq!(stability[0].measured_rounds, 2);
    assert_eq!(stability[0].median_of_medians_ms, Some(1.5));
    assert_eq!(stability[0].min_median_ms, Some(1.0));
    assert_eq!(stability[0].max_median_ms, Some(1.5));
    assert_eq!(stability[0].median_mad_ms, Some(0.5));
    assert_eq!(stability[0].spread_ratio, Some(1.5));
}

#[test]
fn recommendations_select_fastest_correct_gpu_case_with_capacity() {
    let shape = BrowserMathBenchShape::Matmul {
        rows: 256,
        shared: 256,
        cols: 256,
    };
    let cases = vec![
        bench_case(
            "matmul_cpu",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::CpuWasm,
            Some(8.0),
            true,
        ),
        bench_case(
            "matmul_exact",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 256,
                shared: 256,
                cols: 256,
            }),
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(1.5),
            true,
        ),
        bench_case(
            "matmul_capacity",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 512,
                shared: 512,
                cols: 512,
            }),
            BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
            Some(1.0),
            true,
        ),
        bench_case(
            "matmul_wrong",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::WebGpuOneShot,
            Some(0.5),
            false,
        ),
    ];

    let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

    assert_eq!(recommendations.len(), 1);
    let recommendation = &recommendations[0];
    assert_eq!(
        recommendation.reason,
        BrowserMathBenchRecommendationReason::WebGpuFaster
    );
    assert_eq!(
        recommendation.selected_mode,
        Some(BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined)
    );
    assert_eq!(
        recommendation.selected_capacity,
        Some(BrowserMathBenchCapacity::Matmul {
            rows: 512,
            shared: 512,
            cols: 512,
        })
    );
    assert_eq!(
        recommendation.policy_mode,
        Some(BrowserBenchMode::WebGpuPreparedResidentPipelined)
    );
    assert_eq!(
        recommendation.policy_capacity,
        Some(BrowserMathBenchCapacity::Matmul {
            rows: 256,
            shared: 256,
            cols: 256,
        })
    );
    assert_eq!(
        recommendation.policy_reason,
        Some(BrowserMathBenchPolicyReason::MatmulPreparedResidentPipelined)
    );
    assert_eq!(recommendation.policy_matches_selected, Some(false));
    assert_eq!(recommendation.speedup, Some(8.0));
}

#[test]
fn recommendations_use_median_of_repeated_mode_medians() {
    let shape = BrowserMathBenchShape::Matmul {
        rows: 256,
        shared: 256,
        cols: 256,
    };
    let exact = Some(BrowserMathBenchCapacity::Matmul {
        rows: 256,
        shared: 256,
        cols: 256,
    });
    let capacity = Some(BrowserMathBenchCapacity::Matmul {
        rows: 512,
        shared: 512,
        cols: 512,
    });
    let cases = vec![
        bench_case(
            "matmul_cpu_round0",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::CpuWasm,
            Some(8.0),
            true,
        ),
        bench_case(
            "matmul_cpu_round1",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::CpuWasm,
            Some(8.2),
            true,
        ),
        bench_case(
            "matmul_exact_fast_outlier",
            "matmul_f32",
            shape,
            exact,
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(0.5),
            true,
        ),
        bench_case(
            "matmul_exact_slow_round",
            "matmul_f32",
            shape,
            exact,
            BrowserBenchMode::WebGpuPreparedResidentPipelined,
            Some(4.0),
            true,
        ),
        bench_case(
            "matmul_capacity_round0",
            "matmul_f32",
            shape,
            capacity,
            BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
            Some(1.5),
            true,
        ),
        bench_case(
            "matmul_capacity_round1",
            "matmul_f32",
            shape,
            capacity,
            BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
            Some(1.6),
            true,
        ),
    ];

    let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

    assert_eq!(recommendations.len(), 1);
    let recommendation = &recommendations[0];
    assert_eq!(
        recommendation.selected_mode,
        Some(BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined)
    );
    assert_eq!(recommendation.selected_capacity, capacity);
    assert_eq!(recommendation.selected_median_ms, Some(1.6));
    assert_eq!(recommendation.selected_mad_ms, Some(1.6));
    assert_eq!(recommendation.selected_p95_ms, Some(1.6));
    assert_eq!(recommendation.cpu_median_ms, Some(8.2));
    assert_eq!(recommendation.cpu_mad_ms, Some(8.2));
    assert_eq!(recommendation.cpu_p95_ms, Some(8.2));
    assert_eq!(recommendation.speedup, Some(8.2 / 1.6));
}

#[test]
fn recommendations_treat_auto_as_policy_observation_not_candidate() {
    let shape = BrowserMathBenchShape::Matmul {
        rows: 128,
        shared: 128,
        cols: 128,
    };
    let cases = vec![
        bench_case(
            "matmul_cpu",
            "matmul_f32",
            shape,
            None,
            BrowserBenchMode::CpuWasm,
            Some(4.0),
            true,
        ),
        bench_case(
            "matmul_auto",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 128,
                shared: 128,
                cols: 128,
            }),
            BrowserBenchMode::Auto,
            Some(0.5),
            true,
        ),
        bench_case(
            "matmul_auto_pipelined",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 128,
                shared: 128,
                cols: 128,
            }),
            BrowserBenchMode::AutoPipelined,
            Some(0.4),
            true,
        ),
        bench_case(
            "matmul_auto_resident",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 128,
                shared: 128,
                cols: 128,
            }),
            BrowserBenchMode::AutoResidentPipelined,
            Some(0.3),
            true,
        ),
        bench_case(
            "matmul_auto_resident_direct",
            "matmul_f32",
            shape,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 128,
                shared: 128,
                cols: 128,
            }),
            BrowserBenchMode::AutoResidentDirectPipelined,
            Some(0.2),
            true,
        ),
    ];

    let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

    assert_eq!(recommendations.len(), 1);
    assert_eq!(
        recommendations[0].reason,
        BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase
    );
    assert_eq!(
        recommendations[0].selected_mode,
        Some(BrowserBenchMode::CpuWasm)
    );
    assert_eq!(
        recommendations[0].policy_mode,
        Some(BrowserBenchMode::WebGpuPreparedResidentPipelined)
    );
    assert_eq!(recommendations[0].policy_matches_selected, Some(false));
}

#[test]
fn recommendations_keep_cpu_when_gpu_is_not_faster() {
    let shape = BrowserMathBenchShape::Len { len: 65_536 };
    let cases = vec![
        bench_case(
            "add_cpu",
            "tensor_add_f32",
            shape,
            None,
            BrowserBenchMode::CpuWasm,
            Some(0.2),
            true,
        ),
        bench_case(
            "add_gpu",
            "tensor_add_f32",
            shape,
            Some(BrowserMathBenchCapacity::Len { len: 131_072 }),
            BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
            Some(0.8),
            true,
        ),
    ];

    let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

    assert_eq!(recommendations.len(), 1);
    assert_eq!(
        recommendations[0].reason,
        BrowserMathBenchRecommendationReason::CpuFasterOrEqual
    );
    assert_eq!(
        recommendations[0].selected_mode,
        Some(BrowserBenchMode::CpuWasm)
    );
    assert_eq!(
        recommendations[0].policy_mode,
        Some(BrowserBenchMode::CpuWasm)
    );
    assert_eq!(
        recommendations[0].policy_reason,
        Some(BrowserMathBenchPolicyReason::ElementwiseCpuReadbackDominated)
    );
    assert_eq!(recommendations[0].policy_matches_selected, Some(true));
    assert_eq!(recommendations[0].speedup, Some(1.0));
}

const fn large_limits() -> BrowserMathBenchLimits {
    BrowserMathBenchLimits {
        max_storage_buffer_binding_size: 1_u64 << 34,
        max_buffer_size: 1_u64 << 34,
        max_compute_invocations_per_workgroup: 256,
        max_compute_workgroups_per_dimension: 65_535,
    }
}

fn bench_case(
    case_id: &str,
    op: &'static str,
    shape: BrowserMathBenchShape,
    capacity: Option<BrowserMathBenchCapacity>,
    mode: BrowserBenchMode,
    median_ms: Option<f64>,
    passed: bool,
) -> BrowserMathBenchCase {
    BrowserMathBenchCase {
        case_id: case_id.to_owned(),
        op,
        shape,
        capacity,
        mode,
        round_index: 0,
        mode_order_index: 0,
        warmup_iters: 1,
        sample_iters: 1,
        median_ms,
        mad_ms: median_ms,
        min_ms: median_ms,
        p95_ms: median_ms,
        effective_gflops: median_ms
            .filter(|median| *median > 0.0)
            .map(|median| 256.0 / (median * 1_000_000.0)),
        submit_median_ms: None,
        readback_median_ms: None,
        submit_median_share: None,
        readback_median_share: None,
        bytes_uploaded: 0,
        bytes_readback: 0,
        dispatches: usize::from(median_ms.is_some()),
        async_submissions: 0,
        async_readbacks: 0,
        max_in_flight: 0,
        buffer_alloc_count: 0,
        buffer_reuse_count: 0,
        workgroups: 1,
        work_items: 256,
        estimated_flops: 256,
        correctness: BrowserMathBenchCorrectness {
            passed,
            max_abs: 0.0,
            max_rel: 0.0,
        },
        fallback_reason: None,
        checksum: 0.0,
    }
}
