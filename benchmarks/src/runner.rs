use std::{
    future::Future,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use byte_unit::{Byte, Unit};
use ruse_synthesizer::bank::ProgBank;
use ruse_task_parser::{BankConfig, SnythesisTask};
use ruse_ts_synthesizer::TsSynthesizer;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span};

use crate::{config::BenchmarkConfig, results::BenchmarkResult};

#[derive(Debug)]
enum RunnerError {
    Timeout,
    OOM,
}

impl std::error::Error for RunnerError {}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::Timeout => write!(f, "Timeout"),
            RunnerError::OOM => write!(f, "OOM"),
        }
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

async fn watch_vm_usage(max_task_mem: Byte) {
    let proc = procfs::process::Process::myself().unwrap();

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let status = proc.status().unwrap();
        let vmrss = Byte::from_u64_with_unit(status.vmrss.unwrap(), Unit::KiB).unwrap();
        if vmrss > max_task_mem {
            break;
        }
    }
}

async fn watch_for_error<I>(
    timeout: tokio::time::Timeout<I>,
    config: &BenchmarkConfig,
) -> Result<(), RunnerError>
where
    I: Future,
{
    tokio::select! {
        res = timeout => {
            if let Err(_) = res {
                error!(target: "ruse::runner", "Reached timeout");
                Err(RunnerError::Timeout)
            } else {
                Ok(())
            }
        },
        _ = watch_vm_usage(config.max_task_mem) => {
            error!(target: "ruse::runner", "Reached max mem usage");
            Err(RunnerError::OOM)
        }
    }
}

async fn run_synthesizer<P: ProgBank + 'static>(
    synthesizer: &mut TsSynthesizer<P>,
    result: &mut BenchmarkResult,
    max_iterations: u32,
    cancel_token: CancellationToken,
) {
    let _span = info_span!(target: "ruse::runner", "synthesizer_run").entered();

    let start = Instant::now();
    let mut found = None;
    for _ in 0..max_iterations {
        let iteration_start = Instant::now();
        let res = tokio::select! {
            _ = cancel_token.cancelled() => Err(()),
            v = synthesizer.run_iteration() => Ok(v)
        };
        let iteration_took = iteration_start.elapsed();
        if res.is_err() {
            return;
        }
        result.add_iteration(iteration_took, synthesizer.statistics());
        if let Ok(Some(p)) = res {
            found = Some(p);
            break;
        }
    }
    let took = start.elapsed();
    if let Some(found) = &found {
        info!(target: "ruse::runner", "Found \"{}\"", found.get_code());
    } else {
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

fn init_results(task: &SnythesisTask, results: &mut BenchmarkResult) {
    results.set_literals(
        Vec::from_iter(task.string_literals.iter().cloned()),
        Vec::from_iter(task.num_literals.iter().cloned()),
    );
    results.opcode_count = task.opcode_count();
}

pub fn run_task(path: &Path, bench_config: &BenchmarkConfig) -> BenchmarkResult {
    let task_name = PathBuf::from(path.file_name().unwrap());
    let mut result = BenchmarkResult::new(path);

    let _span =
        info_span!(target: "ruse::runner", "task", task_name = task_name.display().to_string())
            .entered();

    let task = match SnythesisTask::from_json_file(path) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to run task {}. {}", task_name.display(), e);
            result.error(&e);
            return result;
        }
    };

    init_results(&task, &mut result);

    let mut synthesizer = match task.get_synthesizer(
        bench_config.max_context_depth,
        bench_config.iteration_workers_count,
        BankConfig {
            bank_type: bench_config.bank_type.into(),
            hash_builder: bench_config.bank_hash_builder,
        },
    ) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to get synthesizer for task {}. {}", task_name.display(), e);
            result.error(&e);
            return result;
        }
    };
    info!(target: "ruse::runner", "Running {}", path.display());
    debug!(target: "ruse::runner", { 
        synthesizer.json = %synthesizer.json_display()
    }, "Benchmark {} Synthesizer", path.display());

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
        if let Err(e) = watch_for_error(timeout, bench_config).await {
            cancel_token.cancel();
            result.error(&e);
            result.add_iteration(Duration::from_secs(0), synthesizer.statistics());
            result.finish(None, bench_config.timeout, synthesizer.statistics());
        }
    });

    drop(runtime);

    result
}
