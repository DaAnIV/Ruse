use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use byte_unit::{Byte, Unit};
use crossbeam_channel::select;
use ruse_synthesizer::{bank::ProgBank, prog::SubProgram};
use ruse_task_parser::SnythesisTask;
use ruse_task_parser::{bank_factory::Bank, error::SnythesisTaskError};
use ruse_ts_synthesizer::TsSynthesizer;
use serde_json::ser::Formatter;
use sysinfo::get_current_pid;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span};

use crate::{
    config::BenchmarkConfig,
    results::{BenchmarkResult, ResultsWriter},
};

#[derive(Debug)]
pub enum RunnerError {
    Timeout,
    OOM,
    CtrlC,
    ForkError,
    ThreadError,
    TaskCreateError(SnythesisTaskError),
    SynthesizerCreateError(SnythesisTaskError),
}

impl std::error::Error for RunnerError {}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::Timeout => write!(f, "Timeout"),
            RunnerError::OOM => write!(f, "OOM"),
            RunnerError::CtrlC => write!(f, "Ctrl+C"),
            RunnerError::ForkError => write!(f, "ForkError"),
            RunnerError::ThreadError => write!(f, "ThreadError"),
            RunnerError::TaskCreateError(e) => write!(f, "TaskCreateError: {}", e),
            RunnerError::SynthesizerCreateError(e) => write!(f, "SynthesizerCreateError: {}", e),
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

async fn watch_vm_usage(max_task_mem: Byte, max_vm_usage: &mut Byte) {
    let proc = procfs::process::Process::myself().unwrap();

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let status = proc.status().unwrap();
        let vmrss = Byte::from_u64_with_unit(status.vmrss.unwrap(), Unit::KiB).unwrap();
        *max_vm_usage = (*max_vm_usage).max(vmrss);
        if vmrss > max_task_mem {
            break;
        }
    }
}

async fn watch_for_error<I>(
    timeout: tokio::time::Timeout<I>,
    config: &BenchmarkConfig,
    max_vm_usage: &mut Byte,
) -> Result<bool, RunnerError>
where
    I: Future<Output = bool>,
{
    tokio::select! {
        res = timeout => {
            match res {
                Ok(v) => Ok(v),
                Err(_) => {
                    error!(target: "ruse::runner", "Reached timeout");
                    Err(RunnerError::Timeout)
                }
            }
        },
        _ = tokio::signal::ctrl_c() => {
            error!(target: "ruse::runner", "Received Ctrl+C!");
            Err(RunnerError::CtrlC)
        },
        _ = watch_vm_usage(config.max_task_mem, max_vm_usage) => {
            error!(target: "ruse::runner", "Reached max mem usage");
            Err(RunnerError::OOM)
        }
    }
}

enum SynthesizerResult {
    Found(Arc<SubProgram>),
    Error(String),
    Cancelled,
    NotFound,
}

async fn run_synthesizer<P: ProgBank + 'static>(
    synthesizer: &mut TsSynthesizer<P>,
    result: &mut BenchmarkResult,
    max_iterations: u32,
    cancel_token: CancellationToken,
) -> bool {
    let _span = info_span!(target: "ruse::runner", "synthesizer_run").entered();

    let start = Instant::now();
    let mut synthesizer_result = SynthesizerResult::NotFound;
    for _ in 0..max_iterations {
        let iteration_start = Instant::now();
        synthesizer_result = tokio::select! {
            _ = cancel_token.cancelled() => SynthesizerResult::Cancelled,
            v = synthesizer.run_iteration() => {
                match v {
                    Ok(Some(p)) => SynthesizerResult::Found(p),
                    Err(e) => SynthesizerResult::Error(e.to_string()),
                    Ok(None) => SynthesizerResult::NotFound,
                }
            }
        };
        let iteration_took = iteration_start.elapsed();
        result.add_iteration(iteration_took, synthesizer.statistics());
        match &synthesizer_result {
            SynthesizerResult::NotFound => (),
            _ => break,
        }
    }
    let took = start.elapsed();
    let found = match synthesizer_result {
        SynthesizerResult::Found(sub_program) => {
            info!(target: "ruse::runner", "Found \"{}\"", sub_program.get_code());
            Some(sub_program)
        }
        SynthesizerResult::Error(e) => {
            error!(target: "ruse::runner", "Error in synthesizer {}", &e);
            result.error_string(&e);
            None
        }
        SynthesizerResult::Cancelled => None,
        SynthesizerResult::NotFound => {
            error!(target: "ruse::runner", "Reached max iterations");
            let err = ReachedMaxIterationError {};
            result.error(&err);
            None
        }
    };

    let success = found.is_some();
    result.finish(found, took, synthesizer.statistics());
    info!(target: "ruse::runner", "Benchmark took {:.3}s", took.as_secs_f32());

    return success;
}

fn get_tokio_runtime(bench_config: &BenchmarkConfig) -> tokio::runtime::Runtime {
    let mut runtime_builder = if bench_config.multi_thread {
        tokio::runtime::Builder::new_multi_thread()
    } else {
        tokio::runtime::Builder::new_current_thread()
    };

    runtime_builder.enable_all().build().unwrap()
}

fn init_results(
    task: &SnythesisTask,
    bench_config: &BenchmarkConfig,
    results: &mut BenchmarkResult,
) {
    results.set_literals(
        Vec::from_iter(task.string_literals.iter().cloned()),
        Vec::from_iter(task.num_literals.iter().cloned()),
    );
    results.set_category(task.category());
    if let Some(source) = task.source() {
        results.set_source(source.clone());
    }
    results.opcode_count = task.opcode_count(bench_config.max_sequence_size);
}

async fn run_task_with_bank<P: ProgBank + 'static>(
    task: SnythesisTask,
    bench_config: &BenchmarkConfig,
    result: &mut BenchmarkResult,
    bank: P,
) -> Result<bool, RunnerError> {
    let task_name = task.name.clone();
    let task_path = task.path.clone();

    let mut synthesizer = match task.get_synthesizer(
        bench_config.max_mutations,
        bench_config.max_sequence_size,
        bench_config.iteration_workers_count,
        bank,
    ) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to get synthesizer for task {}. {}", task_name, e);
            result.error(&e);
            return Err(RunnerError::SynthesizerCreateError(e));
        }
    };
    debug!(target: "ruse::runner", { 
        synthesizer.json = %synthesizer.json_display()
    }, "Benchmark {} Synthesizer", task_path.display());

    if bench_config.dry_run {
        result.finish(None, Duration::from_secs(0), Default::default());
        return Ok(false);
    }

    let cancel_token = synthesizer.get_cancel_token();
    let timeout = tokio::time::timeout(
        bench_config.timeout,
        run_synthesizer(
            &mut synthesizer,
            result,
            bench_config.max_iterations,
            cancel_token.child_token(),
        ),
    );

    let mut max_vm_usage = Byte::from_u64(0);
    let res = watch_for_error(timeout, bench_config, &mut max_vm_usage).await;
    result.set_max_vm_usage(max_vm_usage);
    match res {
        Ok(success) => Ok(success),
        Err(e) => {
            cancel_token.cancel();
            result.error(&e);
            result.add_iteration(Duration::from_secs(0), synthesizer.statistics());
            result.finish(None, bench_config.timeout, synthesizer.statistics());
            Err(e)
        }
    }
}

async fn run_task_async(
    path: &Path,
    bench_config: &BenchmarkConfig,
    result: &mut BenchmarkResult,
) -> Result<bool, RunnerError> {
    let task_name = SnythesisTask::task_name(path);

    let _span = info_span!(target: "ruse::runner", "task", task_name = task_name).entered();

    let task = match SnythesisTask::from_json_file(path) {
        Ok(v) => v,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to parse task {}. {}", task_name, e);
            result.error(&e);
            return Err(RunnerError::TaskCreateError(e));
        }
    };

    init_results(&task, bench_config, result);

    match bench_config.bank_config.new_bank().await {
        Bank::SubsumptionBank(bank) => run_task_with_bank(task, bench_config, result, bank).await,
    }
}

fn run_task_child(
    path: &Path,
    bench_config: &BenchmarkConfig,
    result: &mut BenchmarkResult,
) -> Result<bool, RunnerError> {
    let runtime = get_tokio_runtime(&bench_config);
    runtime.block_on(async {
        tokio::task::block_in_place(|| run_task_async(path, bench_config, result)).await
    })
}

fn ctrl_channel() -> Result<crossbeam_channel::Receiver<()>, ctrlc::Error> {
    let (sender, receiver) = crossbeam_channel::bounded(100);
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })?;

    Ok(receiver)
}

fn run_task_fork<F: Formatter + Sync + Send + Clone + 'static>(
    path: &Path,
    i: usize,
    bench_config: &BenchmarkConfig,
    result_writer: &ResultsWriter<F>,
    sender: crossbeam_channel::Sender<()>,
) -> Result<(), RunnerError> {
    match fork::fork().map_err(|_| RunnerError::ForkError)? {
        fork::Fork::Parent(child_pid) => {
            thread::Builder::new()
                .name("waitchild".into())
                .spawn(move || {
                    fork::waitpid(child_pid).unwrap();
                    let _ = sender.send(());
                })
                .map_err(|_| RunnerError::ForkError)?;
            Ok(())
        }
        fork::Fork::Child => {
            let mut result = BenchmarkResult::new(path);
            debug!(target: "ruse::runner", "Running child {}", get_current_pid().unwrap());

            match run_task_child(path, bench_config, &mut result) {
                Ok(_) => (),
                Err(e) => {
                    result.error(&e);
                }
            }

            result_writer.write_result(&result, i);

            std::process::exit(0);
        }
    }
}

fn run_task_thread<F: Formatter + Sync + Send + Clone + 'static>(
    path: &Path,
    i: usize,
    bench_config: &BenchmarkConfig,
    result_writer: &ResultsWriter<F>,
    sender: crossbeam_channel::Sender<()>,
) -> Result<(), RunnerError> {
    let path_buf = PathBuf::from(path);
    let bench_config_clone = bench_config.clone();
    let result_writer_clone = result_writer.clone();

    thread::Builder::new()
        .name("task_thread".into())
        .spawn(move || {
            let mut result = BenchmarkResult::new(&path_buf);

            match run_task_child(&path_buf, &bench_config_clone, &mut result) {
                Ok(_) => (),
                Err(e) => {
                    result.error(&e);
                }
            }

            result_writer_clone.write_result(&result, i);
            let _ = sender.send(());
        })
        .map_err(|_| RunnerError::ThreadError)?;

    Ok(())
}

pub fn run_task<F: Formatter + Sync + Send + Clone + 'static>(
    path: &Path,
    i: usize,
    bench_config: &BenchmarkConfig,
    result_writer: &ResultsWriter<F>,
) -> Result<crossbeam_channel::Receiver<()>, RunnerError> {
    let (sender, receiver) = crossbeam_channel::bounded(1);

    if bench_config.fork {
        run_task_fork(path, i, bench_config, result_writer, sender)?;
    } else {
        run_task_thread(path, i, bench_config, result_writer, sender)?;
    }

    Ok(receiver)
}

pub fn get_benchmarks_recursively(paths: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let mut all_benchmarks = Vec::new();
    for benchmark in paths {
        if !benchmark.exists() {
            error!(target: "ruse::runner", "Path doesn't exist {}", benchmark.display());
        } else if benchmark.is_dir() {
            for entry in walkdir::WalkDir::new(benchmark)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map(|s| s == "sy").unwrap_or(false)
                })
            {
                all_benchmarks.push(entry.path().to_path_buf());
            }
        } else {
            all_benchmarks.push(benchmark.clone());
        }
    }

    all_benchmarks
}

pub fn run_all_benchmarks<F: Formatter + Sync + Send + Clone + 'static>(
    top_level_benchmark: &[std::path::PathBuf],
    bench_config: &BenchmarkConfig,
    writer: ResultsWriter<F>,
) {
    let mut ctrlc = false;
    let ctrl_c_events = ctrl_channel().expect("Failed to create ctrl_c_events");

    let benchmarks = get_benchmarks_recursively(top_level_benchmark);
    let total_benchmarks = benchmarks.len();
    for (i, benchmark) in benchmarks.into_iter().enumerate() {
        info!(target: "ruse::runner", "Starting benchmark {} [{}/{}]", benchmark.display(), i + 1, total_benchmarks);
        if ctrlc {
            let mut result = BenchmarkResult::new(benchmark.as_path());
            result.error(&RunnerError::CtrlC);
            result.finish(None, Duration::from_secs(0), Default::default());
            writer.write_result(&result, i);
        } else {
            let task_channel = run_task(benchmark.as_path(), i, &bench_config, &writer)
                .expect("Failed to create task channel");
            select! {
                recv(task_channel) -> _ => {}
                recv(ctrl_c_events) -> _ => {
                    ctrlc = true;
                }
            }
        }
    }
}
