use byte_unit::Byte;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use itertools::Itertools;
use ruse_synthesizer::{bank_hasher::BankHasherBuilder, opcode::sort_opcodes};
use ruse_task_parser::SnythesisTask;
use ruse_ts_interpreter::ts_class::TsClassesBuilder;
use serde_json::ser::Formatter;
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    clone::Clone,
    fs::File,
    panic::PanicHookInfo,
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_log::AsTrace;
use tracing_subscriber::{filter::Targets, prelude::*};

mod results;
use clap::Parser;
use config::{BankType, BenchmarkConfig};
use results::ResultsWriter;
use ruse_object_graph::Cache;

mod config;
mod runner;

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

#[derive(Clone, Copy, Debug)]
struct BankKeys(u64, u64);

impl FromStr for BankKeys {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');

        let first = parts.next().unwrap().to_owned();
        let second = parts.next().map(|x| x.to_owned());
        if parts.next().is_some() {
            return Err(anyhow::Error::msg("Value contains more then two ','"));
        }

        let k0: u64 = first.parse()?;
        let k1: u64 = if let Some(next) = second {
            next.parse()?
        } else {
            0
        };

        Ok(Self(k0, k1))
    }
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

    #[arg(long, default_value_t = String::from("100GiB"))]
    max_task_mem: String,

    #[arg(long, default_value_t = 16)]
    workers_count: usize,

    #[arg(long, default_value_t = BankType::SubsumptionBank)]
    bank_type: BankType,

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

    #[arg(long, value_parser = clap::value_parser!(BankKeys))]
    bank_keys: Option<BankKeys>,
}

#[derive(clap::Args, Debug)]
struct PrintOpcodesArgs {
    /// ts files to parse for classes
    #[arg(short, long, num_args(0..))]
    ts_files: Vec<std::path::PathBuf>,

    #[arg(short, long, default_value_t = false)]
    only_ts: bool,

    #[arg(short, long, default_value_t = false)]
    print_summary: bool,
}

// Taken and modified from tracing_panic::panic_hook crate (need the target: ruse)
fn panic_hook(panic_info: &PanicHookInfo) {
    let payload = panic_info.payload();

    #[allow(clippy::manual_map)]
    let payload = if let Some(s) = payload.downcast_ref::<&str>() {
        Some(&**s)
    } else if let Some(s) = payload.downcast_ref::<String>() {
        Some(s.as_str())
    } else {
        None
    };

    let location = panic_info.location().map(|l| l.to_string());
    let (backtrace, note) = {
        let backtrace = Backtrace::capture();
        let note = (backtrace.status() == BacktraceStatus::Disabled)
            .then_some("run with RUST_BACKTRACE=1 environment variable to display a backtrace");
        (Some(backtrace), note)
    };

    tracing::error!(
        target: "ruse",
        {
            panic.payload = payload,
            panic.location = location,
            panic.backtrace = backtrace.map(tracing::field::display),
            panic.note = note,
        },
        "A panic occurred",
    );
}

fn set_logger(cli: &RunArgs) {
    // let fmt_layer = tracing_subscriber::fmt::layer();
    let verbose_filter = Targets::default()
        .with_target("ruse", cli.verbose.log_level_filter().as_trace())
        .with_default(LevelFilter::OFF);

    if let Some(log_path) = &cli.log {
        let file = File::create(log_path).unwrap();
        let file_layer = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_writer(file)
            .json()
            .with_filter(verbose_filter);

        let info_filter = Targets::default()
            .with_target("ruse", LevelFilter::INFO)
            .with_default(LevelFilter::OFF);

        let console_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_filter(info_filter);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer)
            .init();
    } else {
        let console_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_filter(verbose_filter);

        tracing_subscriber::registry().with(console_layer).init();
    }

    std::panic::set_hook(Box::new(panic_hook));
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

fn construct_config(cli: &RunArgs) -> BenchmarkConfig {
    let bank_hash_builder = match cli.bank_keys {
        Some(keys) => BankHasherBuilder::new_with_keys(keys.0, keys.1),
        None => BankHasherBuilder::new_with_random_keys(),
    };

    BenchmarkConfig {
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
        multi_thread: !cli.single_thread,
        max_context_depth: cli.max_context_depth,
        iteration_workers_count: cli.workers_count,
        benchmarks: cli.benchmarks.clone(),
        max_task_mem: Byte::parse_str(&cli.max_task_mem, true).unwrap(),
        bank_type: cli.bank_type,
        bank_hash_builder,
    }
}

fn run_benchmarks(cli: &RunArgs) -> ExitCode {
    set_logger(&cli);

    let bench_config = construct_config(cli);

    let max_task_mem = bench_config.max_task_mem;
    info!(target: "ruse::runner", "PID: {}", std::process::id());
    info!(target: "ruse::runner", "Timeout {:.3} seconds", bench_config.timeout.as_secs_f32());
    info!(target: "ruse::runner", "Max task mem {}", format!("{max_task_mem:#}"));
    info!(target: "ruse::runner", "Max iterations: {}", bench_config.max_iterations);
    info!(target: "ruse::runner", "Max context depth: {}", bench_config.max_context_depth);
    info!(target: "ruse::runner", "Workers count: {}", bench_config.iteration_workers_count);
    info!(target: "ruse::runner", "Bank hash keys: {}", bench_config.bank_hash_builder);

    if cli.tokio_console {
        console_subscriber::init();
    }

    let results_path = get_result_path(&cli);

    if cli.pretty {
        run_all_benchmarks(
            &cli,
            &bench_config,
            ResultsWriter::from_path_pretty(results_path.as_path(), &bench_config),
        );
    } else {
        run_all_benchmarks(
            &cli,
            &bench_config,
            ResultsWriter::from_path(results_path.as_path(), &bench_config),
        );
    }

    info!(target: "ruse::runner", "Results written to {:?}", results_path.as_path());

    ExitCode::SUCCESS
}

fn print_opcodes(cli: &PrintOpcodesArgs) -> ExitCode {
    let cache = Arc::new(Cache::new());
    let mut builder = TsClassesBuilder::new();
    let mut class_names = vec![];

    for full_path in cli.ts_files.iter() {
        class_names.extend(builder.add_ts_files(&full_path, &cache).unwrap());
    }

    let classes = builder.finalize(&cache);

    let composite_opcodes = if cli.only_ts {
        SnythesisTask::get_classes_opcodes(&classes, &class_names)
    } else {
        SnythesisTask::get_composite_opcodes(&classes, &class_names, true, &cache)
    };
    let opcodes_len = composite_opcodes.len();

    let sorted_opcodes = sort_opcodes(composite_opcodes);

    println!("Composite opcodes:");
    sorted_opcodes.iter().for_each(|(_, ops)| {
        ops.iter().for_each(|op| println!("{}", op.op_name()));
    });

    if cli.print_summary {
        println!();
        println!("summary:");
        sorted_opcodes.iter().for_each(|(arg_types, ops)| {
            println!(
                "{0: <30} {1: <10}",
                arg_types.iter().map(|x| x.to_string()).join(",") + ":",
                ops.len()
            );
        });
        println!("{0: <30} {1: <10}", "All:", opcodes_len);
    } else {
        println!("Opcodes count = {}", opcodes_len);
    }

    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        RuseCommands::Run(args) => run_benchmarks(&args),
        RuseCommands::PrintOpcodes(args) => print_opcodes(&args),
    }
}
