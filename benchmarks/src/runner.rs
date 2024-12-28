use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use ruse_object_graph::Cache;
use ruse_ts_synthesizer::TsSynthesizer;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{config::BenchmarkConfig, results::BenchmarkResult, task};

struct TimeoutError {}
impl std::error::Error for TimeoutError {}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Timeout")
    }
}
impl std::fmt::Debug for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeoutError").finish()
    }
}

struct ReachedMaxIterationError {}
impl std::error::Error for ReachedMaxIterationError {}

impl std::fmt::Display for ReachedMaxIterationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReachedMaxIterationError")
    }
}
impl std::fmt::Debug for ReachedMaxIterationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachedMaxIterationError").finish()
    }
}

async fn run_synthesizer(
    synthesizer: &mut TsSynthesizer,
    result: &mut BenchmarkResult,
    max_iterations: u32,
    cancel_token: CancellationToken,
) {
    let start = Instant::now();
    let mut found = None;
    for _ in 0..max_iterations {
        let iteration_start = Instant::now();
        let res = tokio::select! {
            _ = cancel_token.cancelled() => Err(TimeoutError {}),
            v = synthesizer.run_iteration() => Ok(v)
        };
        let iteration_took = iteration_start.elapsed();
        if let Err(e) = res {
            result.error(&e);
            return;
        }
        result.add_iteration(iteration_took, synthesizer.statistics());
        if let Ok(Some(p)) = res {
            found = Some(p);
            break;
        }
    }
    let took = start.elapsed();
    if found.is_none() {
        error!(target: "ruse::runner", "Reached max iterations");
        let err = ReachedMaxIterationError {};
        result.error(&err);
    }

    result.finish(found, took, synthesizer.statistics());
    info!(target: "ruse::runner", "Benchmark took {:.3}s", took.as_secs_f32());
}

fn get_tokio_runtime(bench_config: &BenchmarkConfig) -> tokio::runtime::Runtime {
    let mut runtime_builder = if bench_config.multi_thread {
        tokio::runtime::Builder::new_multi_thread()
    } else {
        tokio::runtime::Builder::new_current_thread()
    };

    runtime_builder.enable_all().build().unwrap()
}

pub fn run_task(path: &Path, cache: Arc<Cache>, bench_config: &BenchmarkConfig) -> BenchmarkResult {
    let task_name = PathBuf::from(path.file_name().unwrap());
    let mut result = BenchmarkResult::new(path);

    let task = match task::SnythesisTask::from_json_file(path, &cache) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to parse task for {}. {}", task_name.display(), e);
            result.error(&e);
            return result;
        }
    };

    task.populate_results(&mut result);

    let mut synthesizer = match task.get_synthesizer(
        bench_config.max_context_depth,
        bench_config.iteration_workers_count,
        &cache,
    ) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to get synthesizer for task {}. {}", task_name.display(), e);
            result.error(&e);
            return result;
        }
    };
    info!(target: "ruse::runner", "Running {}", path.display());

    let runtime = get_tokio_runtime(bench_config);

    runtime.block_on(async {
        let cancel_token = synthesizer.get_cancel_token();
        let timeout = tokio::time::timeout(
            bench_config.timeout,
            run_synthesizer(
                &mut synthesizer,
                &mut result,
                bench_config.max_iterations,
                cancel_token.child_token(),
            ),
        );
        if let Err(e) = timeout.await {
            error!(target: "ruse::runner", "Reached timeout");
            cancel_token.cancel();
            result.error(&e);
            result.add_iteration(Duration::from_secs(0), synthesizer.statistics());
            result.finish(None, bench_config.timeout, synthesizer.statistics());
        }
    });

    result
}
