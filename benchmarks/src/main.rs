use clap_verbosity_flag::{InfoLevel, Verbosity};
use ruse_ts_interpreter::ts_class::TsClasses;
use serde_json::ser::Formatter;
use std::{clone::Clone, fs::File, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};
use task::SnythesisTask;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_log::AsTrace;
use tracing_subscriber::{filter::Targets, prelude::*};

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

fn get_result_path(cli: &RunArgs) -> PathBuf {
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
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: RuseCommands,
}

#[derive(clap::Subcommand, Debug)]
enum RuseCommands {
    Run(RunArgs),
    PrintOpcodes(PrintOpcodesArgs),
}

#[derive(clap::Args, Debug)]
struct RunArgs {
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

    #[arg(long, default_value_t = 5)]
    max_context_depth: usize,
    
    #[arg(long, default_value_t = 16)]
    workers_count: usize,
    
    #[arg(long, default_value_t = 4096)]
    chunk_size: usize,

    #[arg(long, default_value_t = false)]
    print_all_programs: bool,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,

    #[arg(long, default_value_t = false)]
    single_thread: bool,

    #[arg(long, default_value_t = false)]
    tokio_console: bool,

    /// Results are saved pretty json
    #[arg(long)]
    log: Option<std::path::PathBuf>,

    /// Results are saved pretty json
    #[arg(long, default_value_t = false)]
    pretty: bool,
}

#[derive(clap::Args, Debug)]
struct PrintOpcodesArgs {
    /// ts files to parse for classes
    #[arg(short, long, num_args(0..))]
    ts_files: Vec<std::path::PathBuf>,
}

fn set_logger(cli: &RunArgs) {
    // let fmt_layer = tracing_subscriber::fmt::layer();
    let verbose_filter = Targets::default()
        .with_target("ruse", cli.verbose.log_level_filter().as_trace())
        .with_default(LevelFilter::OFF);

    if let Some(log_path) = &cli.log {
        let file = File::create(log_path).unwrap();
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file)
            .json()
            .with_filter(verbose_filter);

        let info_filter = Targets::default()
            .with_target("ruse", LevelFilter::INFO)
            .with_default(LevelFilter::OFF);

        let console_layer = tracing_subscriber::fmt::layer().with_filter(info_filter);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer)
            .init();
    } else {
        let console_layer = tracing_subscriber::fmt::layer().with_filter(verbose_filter);

        tracing_subscriber::registry().with(console_layer).init();
    }
}

fn run_all_benchmarks<F>(
    cli: &RunArgs,
    bench_config: &config::BenchmarkConfig,
    mut writer: ResultsWriter<F>,
) where
    F: Formatter + Clone,
{
    for benchmark in &cli.benchmarks {
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

fn run_benchmarks(cli: &RunArgs) -> ExitCode {
    set_logger(&cli);

    let bench_config = BenchmarkConfig {
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
        multi_thread: !cli.single_thread,
        max_context_depth: cli.max_context_depth,
        iteration_workers_count: cli.workers_count,
        iteration_chunk_size: cli.chunk_size,
    };

    info!(target: "ruse::runner", "Timeout {:.3} seconds", bench_config.timeout.as_secs_f32());
    info!(target: "ruse::runner", "Max iterations: {}", bench_config.max_iterations);
    info!(target: "ruse::runner", "Workers count: {}", bench_config.iteration_workers_count);
    info!(target: "ruse::runner", "Chunk size: {}", bench_config.iteration_chunk_size);

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

fn print_opcodes(cli: &PrintOpcodesArgs) -> ExitCode {
    let cache = Cache::new();
    let classes = TsClasses::new();
    let mut class_names = vec![];

    class_names.extend(SnythesisTask::add_classes_from_ts_files(
        &classes,
        cli.ts_files.iter().cloned(),
        &cache,
    ).unwrap());

    let composite_opcodes = SnythesisTask::get_composite_opcodes(&classes, &class_names, &cache);

    composite_opcodes.iter().for_each(|op| {
        println!("{}", op.op_name());
    });

    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        RuseCommands::Run(args) => run_benchmarks(&args),
        RuseCommands::PrintOpcodes(args) => print_opcodes(&args),
    }
}
