use crate::perf_suite_report::{BenchmarkConfig, BenchmarkTargetResult};
use axiom_apps::gateway;
use axiom_core::{
    AgentState, DomainEvent, Effect, Intent, build_policy_audit, decide, evaluate_policy, reduce,
};
use std::hint::black_box;
use std::time::Instant;

pub(crate) fn benchmark_core_reduce_path(config: &BenchmarkConfig) -> BenchmarkTargetResult {
    let initial = AgentState::default();
    let intent = Intent::write(
        "bench-intent",
        Some(String::from("system")),
        "bench-key",
        "bench-value",
    );
    let verdict = evaluate_policy(&initial, &intent);
    let decision = decide(&intent, &verdict);
    let audit = build_policy_audit(&initial, &intent, &verdict);

    let mut effects = Vec::with_capacity(config.records);
    for i in 0..config.records {
        effects.push(Effect::PutFact {
            key: format!("core-key-{i}"),
            value: format!("core-value-{i}"),
        });
    }

    let events = vec![
        DomainEvent::IntentAccepted { intent },
        DomainEvent::PolicyEvaluated { audit },
        DomainEvent::DecisionCalculated { decision },
        DomainEvent::EffectsApplied { effects },
    ];

    measure_target(
        "core_reduce_path",
        config,
        config.records.saturating_add(4) as u64,
        || {
            let mut state = AgentState::default();
            for event in &events {
                state = reduce(&state, event);
            }

            black_box(
                state
                    .revision
                    .wrapping_add(state.audit_count)
                    .wrapping_add(state.denied_count)
                    .wrapping_add(state.facts.len() as u64),
            )
        },
    )
}

pub(crate) fn benchmark_memory_recall_path(config: &BenchmarkConfig) -> BenchmarkTargetResult {
    let mut state = AgentState::default();
    for i in 0..config.records {
        state.facts.insert(memory_key(i), format!("value-{i:06}"));
    }

    let mut recall_keys = Vec::with_capacity(config.records);
    for i in 0..config.records {
        if i % 2 == 0 {
            recall_keys.push(memory_key(i));
        } else {
            recall_keys.push(format!("missing-key-{i:06}"));
        }
    }

    measure_target("memory_recall_path", config, config.records as u64, || {
        let mut checksum = 0_u64;
        for key in &recall_keys {
            match state.facts.get(key) {
                Some(value) => {
                    checksum = checksum.wrapping_add(value.len() as u64);
                }
                None => {
                    checksum = checksum.wrapping_add(1);
                }
            }
        }
        black_box(checksum)
    })
}

pub(crate) fn benchmark_gateway_validation_request_path(
    config: &BenchmarkConfig,
) -> BenchmarkTargetResult {
    let requests = build_gateway_requests(config.records);
    let mut runtime = gateway::GatewayRuntime::new();

    measure_target(
        "gateway_validation_request_path",
        config,
        config.records as u64,
        || {
            let mut checksum = 0_u64;
            for request in &requests {
                let response = runtime.handle(request.clone());
                checksum = checksum.wrapping_add(u64::from(response.status_code));
                checksum = checksum.wrapping_add(response.state.revision);
                if response.processed() {
                    checksum = checksum.wrapping_add(1);
                }
            }
            black_box(checksum)
        },
    )
}

fn build_gateway_requests(records: usize) -> Vec<gateway::HttpBoundaryRequest> {
    let mut requests = Vec::with_capacity(records);

    for i in 0..records {
        let body = match i % 3 {
            0 => format!("write:bench-key-{i}=bench-value-{i}"),
            1 => format!("read:bench-key-{i}"),
            _ => format!("remove:bench-key-{i}"),
        };

        let source_ip = if i % 2 == 0 { "10.0.0.8" } else { "127.0.0.1" };

        requests.push(gateway::HttpBoundaryRequest::new(
            gateway::GATEWAY_METHOD,
            gateway::GATEWAY_PATH,
            &body,
            source_ip,
        ));
    }

    requests
}

fn memory_key(index: usize) -> String {
    format!("memory-key-{index:06}")
}

fn measure_target(
    name: &'static str,
    config: &BenchmarkConfig,
    operations_per_iteration: u64,
    mut run_iteration: impl FnMut() -> u64,
) -> BenchmarkTargetResult {
    let mut warmup_checksum = 0_u64;
    for _ in 0..config.warmup {
        warmup_checksum = warmup_checksum.wrapping_add(run_iteration());
    }
    black_box(warmup_checksum);

    let start = Instant::now();
    let mut checksum = 0_u64;
    let mut iteration_durations_ns = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        let iteration_start = Instant::now();
        checksum = checksum.wrapping_add(run_iteration());
        iteration_durations_ns.push(iteration_start.elapsed().as_nanos());
    }
    let elapsed_ns = start.elapsed().as_nanos();
    black_box(checksum);

    iteration_durations_ns.sort_unstable();
    let p50_ns_per_iteration = percentile_ns(&iteration_durations_ns, 50);
    let p95_ns_per_iteration = percentile_ns(&iteration_durations_ns, 95);

    let operations_per_iteration_u128 = u128::from(operations_per_iteration);
    let operations = operations_per_iteration.saturating_mul(config.iterations as u64);
    let avg_ns_per_iteration = elapsed_ns / config.iterations as u128;
    let p50_ns_per_operation = if operations_per_iteration == 0 {
        0
    } else {
        p50_ns_per_iteration / operations_per_iteration_u128
    };
    let p95_ns_per_operation = if operations_per_iteration == 0 {
        0
    } else {
        p95_ns_per_iteration / operations_per_iteration_u128
    };
    let avg_ns_per_operation = if operations == 0 {
        0
    } else {
        elapsed_ns / u128::from(operations)
    };

    let ops_per_sec = if elapsed_ns == 0 {
        0
    } else {
        let value = (u128::from(operations) * 1_000_000_000_u128) / elapsed_ns;
        u64::try_from(value).unwrap_or(u64::MAX)
    };

    BenchmarkTargetResult {
        name,
        operations,
        warmup_iterations: config.warmup,
        measured_iterations: config.iterations,
        elapsed_ns,
        avg_ns_per_iteration,
        p50_ns_per_iteration,
        p95_ns_per_iteration,
        p50_ns_per_operation,
        p95_ns_per_operation,
        avg_ns_per_operation,
        ops_per_sec,
        checksum,
    }
}

fn percentile_ns(sorted_samples_ns: &[u128], percentile: usize) -> u128 {
    if sorted_samples_ns.is_empty() {
        return 0;
    }

    let clamped_percentile = percentile.min(100);
    let max_index = sorted_samples_ns.len() - 1;
    let index = (max_index
        .saturating_mul(clamped_percentile)
        .saturating_add(99))
        / 100;
    sorted_samples_ns[index]
}

#[cfg(test)]
mod tests {
    use super::percentile_ns;

    #[test]
    fn percentile_ns_returns_zero_for_empty_samples() {
        assert_eq!(percentile_ns(&[], 50), 0);
    }

    #[test]
    fn percentile_ns_uses_deterministic_ceil_indexing() {
        let mut samples = vec![40_u128, 10, 30, 20];
        samples.sort_unstable();

        assert_eq!(percentile_ns(&samples, 50), 30);
        assert_eq!(percentile_ns(&samples, 95), 40);
        assert_eq!(percentile_ns(&samples, 100), 40);
    }
}
