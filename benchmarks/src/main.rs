use std::{
    path::Path,
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

mod results;
use clap::Parser;
use config::BenchmarkConfig;
use results::ResultsWriter;
use ruse_object_graph::Cache;
use ruse_ts_synthesizer::TsSynthesizer;

use crate::results::BenchmarkResult;
mod config;
mod task;
#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn get_writer(config: &config::BenchmarkConfig) -> ResultsWriter {
    if config.output.is_dir() {
        std::fs::create_dir_all(config.output.as_path()).expect("Failed to create output dir");
        let mut path = config.output.clone();
        path.push(format!(
            "benchmarks_{}.json",
            chrono::Local::now().format("%Y-%m-%d-%H:%M:%S%.f")
        ));
        ResultsWriter::from_path(&path)
    } else {
        ResultsWriter::from_path(&config.output)
    }
}

/// Run benchmarks on the ruse synthesizer
#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// An .sy benchmark file or directory containing benchmark files
    #[arg(short, long, num_args(1..))]
    benchmarks: Vec<std::path::PathBuf>,

    /// Saves the results in this directory
    #[arg(short, long)]
    output: std::path::PathBuf,

    /// Timeout per benchmark in seconds
    #[arg(short, long, default_value_t = 300)]
    timeout: u64,

    /// Timeout per benchmark in seconds
    #[arg(short, long, default_value_t = 5)]
    max_iterations: u32,

    #[arg(long, default_value_t = false)]
    print_all_programs: bool,

    #[arg(long, default_value_t = false)]
    single_thread: bool,

    #[arg(long, default_value_t = false)]
    tokio_console: bool,
}

struct TimeoutError {}

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

impl std::error::Error for TimeoutError {}

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
    result.finish(found, took, synthesizer.statistics());
    println!("Benchmark took {:.3}s", took.as_secs_f32());
}

fn get_tokio_runtime(bench_config: &BenchmarkConfig) -> tokio::runtime::Runtime {
    let mut runtime_builder = if bench_config.multi_thread {
        tokio::runtime::Builder::new_multi_thread()
    } else {
        tokio::runtime::Builder::new_current_thread()
    };

    runtime_builder.enable_all().build().unwrap()
}

fn run_task(path: &Path, cache: Arc<Cache>, bench_config: &BenchmarkConfig) -> BenchmarkResult {
    let task_name = path.file_name().unwrap().to_str().unwrap();
    let mut result = BenchmarkResult::new(task_name);

    // println!("{}", f_name);
    let task = match task::SnythesisTask::from_json_file(path, &cache) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse task for {}. {}", task_name, e);
            result.error(&e);
            return result;
        }
    };
    let mut synthesizer = match task.get_synthesizer(&cache) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to get synthesizer for task {}. {}", task_name, e);
            result.error(&e);
            return result;
        }
    };
    println!("{}", task_name);

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
            eprintln!("Reached timeout");
            cancel_token.cancel();
            result.error(&e);
            result.add_iteration(Duration::from_secs(0), synthesizer.statistics());
            result.finish(None, bench_config.timeout, synthesizer.statistics());
        }
    });

    if bench_config.print_inserted_programs {
        synthesizer.print_all_programs()
    }

    result
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let bench_config = BenchmarkConfig {
        benchmarks: cli.benchmarks,
        output: cli.output,
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
        print_inserted_programs: cli.print_all_programs,
        multi_thread: !cli.single_thread,
    };
    print!("{}", bench_config);

    if cli.tokio_console {
        console_subscriber::init();
    }

    let mut writer = get_writer(&bench_config);

    for benchmark in &bench_config.benchmarks {
        if !benchmark.exists() {
            eprintln!("Path doesn't exist {}", benchmark.to_str().unwrap());
        } else if benchmark.is_dir() {
            for entry in walkdir::WalkDir::new(benchmark)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file() && e.file_name().to_str().unwrap().ends_with(".sy")
                })
            {
                let cache = Arc::new(Cache::new());
                let result = run_task(entry.path(), cache.clone(), &bench_config);
                writer.write_result(&result);
            }
        } else {
            let cache = Arc::new(Cache::new());
            let result = run_task(benchmark.as_path(), cache.clone(), &bench_config);
            writer.write_result(&result);
        }
    }

    println!("Results written to {:?}", writer.path());

    ExitCode::SUCCESS
}
