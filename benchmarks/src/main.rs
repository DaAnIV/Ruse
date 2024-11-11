use clap_verbosity_flag::{InfoLevel, Verbosity};
use serde_json::ser::Formatter;
use std::{clone::Clone, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};
use tracing::{debug, error, info};
use tracing_log::AsTrace;

mod results;
use clap::Parser;
use config::BenchmarkConfig;
use results::ResultsWriter;
use ruse_object_graph::Cache;

mod config;
mod runner;
mod task;

#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn get_result_path(cli: &Cli) -> PathBuf {
    if cli.output.is_dir() {
        std::fs::create_dir_all(cli.output.as_path()).expect("Failed to create output dir");
        let mut path = cli.output.clone();
        path.push(format!(
            "benchmarks_{}.json",
            chrono::Local::now().format("%Y-%m-%d-%H:%M:%S%.f")
        ));
        path
    } else {
        cli.output.clone()
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

    /// Max number of synthesizer iterations
    #[arg(short, long, default_value_t = 5)]
    max_iterations: u32,

    #[arg(long, default_value_t = false)]
    print_all_programs: bool,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,

    #[arg(long, default_value_t = false)]
    single_thread: bool,

    #[arg(long, default_value_t = false)]
    tokio_console: bool,

    /// Results are saved pretty json
    #[arg(long, default_value_t = false)]
    pretty: bool,
}

fn set_logger(cli: &Cli) {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(cli.verbose.log_level_filter().as_trace())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap()
}

fn run_all_benchmarks<F>(cli: &Cli, bench_config: &config::BenchmarkConfig, mut writer: ResultsWriter<F>)
where
    F: Formatter + Clone,
{
    for benchmark in &cli.benchmarks {
        if !benchmark.exists() {
            error!(target: "ruse::runner", "Path doesn't exist {}", benchmark.to_str().unwrap());
        } else if benchmark.is_dir() {
            for entry in walkdir::WalkDir::new(benchmark)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file() && e.file_name().to_str().unwrap().ends_with(".sy")
                })
            {
                let cache = Arc::new(Cache::new());
                let result = runner::run_task(entry.path(), cache.clone(), &bench_config);
                writer.write_result(&result);
            }
        } else {
            let cache = Arc::new(Cache::new());
            let result = runner::run_task(benchmark.as_path(), cache.clone(), &bench_config);
            writer.write_result(&result);
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    set_logger(&cli);

    let bench_config = BenchmarkConfig {
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
        multi_thread: !cli.single_thread,
    };

    debug!(target: "ruse::runner", "{}", bench_config);

    if cli.tokio_console {
        console_subscriber::init();
    }

    let results_path = get_result_path(&cli);

    if cli.pretty {
        run_all_benchmarks(
            &cli,
            &bench_config,
            ResultsWriter::from_path_pretty(results_path.as_path()),
        );
    } else {
        run_all_benchmarks(
            &cli,
            &bench_config,
            ResultsWriter::from_path(results_path.as_path()),
        );
    }

    info!(target: "ruse::runner", "Results written to {:?}", results_path.as_path());

    ExitCode::SUCCESS
}
