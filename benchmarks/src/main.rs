use std::{
    ops::DerefMut,
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
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

fn get_writer(config: &config::BenchmarkConfig) -> ResultsWriter {
    std::fs::create_dir_all(config.output_dir.as_path()).expect("Failed to create output dir");
    let mut path = config.output_dir.clone();
    path.push(format!(
        "benchmarks_{}.json",
        chrono::Local::now().format("%Y-%m-%d-%H:%M:%S%.f")
    ));
    ResultsWriter::from_path(path)
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
    cache: Arc<Cache>,
    max_iterations: u32,
    cancel_token: CancellationToken,
) {
    let start = Instant::now();
    let mut found = None;
    for _ in 0..max_iterations {
        let iteration_start = Instant::now();
        let res = tokio::select! {
            _ = cancel_token.cancelled() => Err(TimeoutError {}),
            v = synthesizer.run_iteration(&cache) => Ok(v)
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

async fn run_task(
    task_name: &str,
    task: task::SnythesisTask,
    cache: Arc<Cache>,
    bench_config: &BenchmarkConfig,
) -> BenchmarkResult {
    let mut result = BenchmarkResult::new(task_name);

    let mut synthesizer = match task.get_synthesizer(&cache) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to get synthesizer for task {}. {}", task_name, e);
            result.error(&e);
            return result;
        }
    };
    println!("{}", task_name);

    let cancel_token = synthesizer.get_cancel_token();
    let timeout = tokio::time::timeout(
        bench_config.timeout,
        run_synthesizer(
            &mut synthesizer,
            &mut result,
            cache,
            bench_config.max_iterations,
            cancel_token.child_token(),
        ),
    );
    if let Err(_) = timeout.await {
        eprintln!("Reached timeout");
        cancel_token.cancel();
    }

    return result;
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let bench_config = BenchmarkConfig {
        benchmarks: cli.benchmarks,
        output_dir: cli.output,
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
    };
    print!("{}", bench_config);

    let mut writer = get_writer(&bench_config);

    for benchmark in &bench_config.benchmarks {
        if benchmark.is_dir() {
            for entry in walkdir::WalkDir::new(benchmark)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file() && e.file_name().to_str().unwrap().ends_with(".sy")
                })
            {
                let cache = Arc::new(Cache::new());
                let f_name = String::from(entry.file_name().to_string_lossy());
                // println!("{}", f_name);
                let task = task::SnythesisTask::from_json_file(entry.path(), &cache).unwrap();
                let result = run_task(&f_name, task, cache.clone(), &bench_config).await;
                writer.write_result(&result);
            }
        } else {
            let cache = Arc::new(Cache::new());
            let f_name = String::from(benchmark.file_name().unwrap().to_string_lossy());
            let task = task::SnythesisTask::from_json_file(benchmark.as_path(), &cache).unwrap();
            let result = run_task(&f_name, task, cache.clone(), &bench_config).await;
            writer.write_result(&result);
        }
    }

    ExitCode::SUCCESS
}
