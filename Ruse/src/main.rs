use byte_unit::Byte;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use itertools::Itertools;
use ruse_synthesizer::opcode::sort_opcodes;
use ruse_task_parser::SnythesisTask;
use ruse_ts_interpreter::ts_classes::{TsClassesBuilder, TsClassesBuilderOptions};
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    clone::Clone,
    fs::File,
    panic::PanicHookInfo,
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_log::AsTrace;
use tracing_subscriber::{filter::Targets, prelude::*};

mod results;
use clap::Parser;
use config::BenchmarkConfig;
use results::ResultsWriter;

mod config;
mod runner;

#[cfg(feature = "mimalloc")]
mod mimalloc {
    use mimalloc::MiMalloc;

    #[global_allocator]
    static GLOBAL: MiMalloc = MiMalloc;
}

use crate::config::{BankArgs, BankTypeArg};

fn get_result_path(cli: &RunArgs) -> Result<PathBuf, String> {
    if cli.output.is_file() {
        return Err(format!("Output path {} is a file", cli.output.display()));
    } else if cli.output.exists() {
        let mut path = cli.output.clone();
        path.push(format!(
            "benchmarks_{}",
            chrono::Local::now().format("%Y-%m-%d-%H:%M:%S%.f")
        ));
        Ok(path)
    } else {
        Ok(cli.output.clone())
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
    #[arg(
        short,
        long,
        default_value_t = 300,
        help = "Timeout per benchmark in seconds"
    )]
    timeout: u64,

    /// Max number of synthesizer iterations
    #[arg(short, long, default_value_t = 5)]
    max_iterations: u32,

    #[arg(long, default_value_t = 5)]
    max_mutations: u32,

    #[arg(long, default_value_t = String::from("100GiB"))]
    max_task_mem: String,

    #[arg(long, default_value_t = 16)]
    workers_count: usize,

    #[arg(long, default_value_t = 2)]
    max_sequence_size: usize,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,

    #[arg(long, default_value_t = false, hide = true)]
    single_thread: bool,

    #[arg(long, default_value_t = false, hide = true)]
    tokio_console: bool,

    /// Results are saved pretty json
    #[arg(long)]
    log: Option<std::path::PathBuf>,

    #[arg(long, value_enum, default_value_t = BankTypeArg::SubsumptionBank)]
    bank_type: BankTypeArg,

    #[arg(long, action = clap::ArgAction::Append, allow_hyphen_values = true, help = BankArgs::help(), next_line_help=true)]
    bank_arg: Vec<String>,

    #[arg(
        long,
        default_value_t = false,
        help = "Don't run benchmarks, just parse them"
    )]
    dry_run: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Run benchmarks in the same process without forking"
    )]
    no_fork: bool,

    #[arg(long, default_value_t = false, help = "Don't sleep between benchmarks")]
    no_sleep: bool,

    #[arg(
        long,
        default_value_t = 5,
        help = "Sleep time between benchmarks in seconds"
    )]
    sleep_time: u64,
}

#[derive(clap::Args, Debug)]
struct PrintOpcodesArgs {
    /// ts files to parse for classes
    #[arg(short, long, num_args(0..))]
    ts_files: Vec<std::path::PathBuf>,

    #[arg(short, long, default_value_t = false)]
    only_ts: bool,

    #[arg(long, default_value_t = 2)]
    max_sequence_size: usize,

    #[arg(long, default_value_t = false)]
    ignore_string_ops: bool,

    #[arg(short, long, default_value_t = false)]
    print_summary: bool,

    #[arg(long, default_value_t = false)]
    print_js_code: bool,
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

fn construct_config(cli: &RunArgs) -> BenchmarkConfig {
    let bank_args = BankArgs::parse_by_bank_type(cli.bank_type, &cli.bank_arg);

    let sleep = if cli.no_sleep || cli.dry_run {
        None
    } else {
        Some(Duration::from_secs(cli.sleep_time))
    };

    BenchmarkConfig {
        timeout: Duration::from_secs(cli.timeout),
        max_iterations: cli.max_iterations,
        multi_thread: !cli.single_thread,
        max_mutations: cli.max_mutations,
        iteration_workers_count: cli.workers_count,
        max_sequence_size: cli.max_sequence_size,
        benchmarks: cli.benchmarks.clone(),
        max_task_mem: Byte::parse_str(&cli.max_task_mem, true).unwrap(),
        bank_config: bank_args.into(),
        dry_run: cli.dry_run,
        fork: !cli.no_fork && !cli.tokio_console,
        sleep,
    }
}

fn run_benchmarks(cli: &RunArgs) -> ExitCode {
    if cli.tokio_console {
        console_subscriber::init();
    } else {
        set_logger(&cli);
    }

    let bench_config = construct_config(cli);

    let max_task_mem = bench_config.max_task_mem;
    info!(target: "ruse::runner", "start time: {}", chrono::Local::now());
    info!(target: "ruse::runner", "CMD: {}", std::env::args().join(" "));
    info!(target: "ruse::runner", "PID: {}", std::process::id());
    info!(target: "ruse::runner", "Timeout {:.3} seconds", bench_config.timeout.as_secs_f32());
    info!(target: "ruse::runner", "Max task mem {}", format!("{max_task_mem:#}"));
    info!(target: "ruse::runner", "Max iterations: {}", bench_config.max_iterations);
    info!(target: "ruse::runner", "Max mutations: {}", bench_config.max_mutations);
    info!(target: "ruse::runner", "Workers count: {}", bench_config.iteration_workers_count);
    info!(target: "ruse::runner", "Bank config {}", bench_config.bank_config);
    info!(target: "ruse::runner", "Max sequence size: {}", bench_config.max_sequence_size);
    #[cfg(feature = "mimalloc")]
    info!(target: "ruse::runner", "Using mimalloc allocator");
    #[cfg(not(feature = "mimalloc"))]
    info!(target: "ruse::runner", "Using default system allocator");

    let results_path = match get_result_path(&cli) {
        Ok(path) => path,
        Err(e) => {
            error!(target: "ruse::runner", "Failed to get results path: {}", e);
            return ExitCode::FAILURE;
        }
    };

    runner::run_all_benchmarks(
        &cli.benchmarks,
        &bench_config,
        ResultsWriter::from_path_pretty(results_path.as_path(), &bench_config),
    );

    info!(target: "ruse::runner", "Results written to {:?}", results_path.as_path());

    ExitCode::SUCCESS
}

fn print_opcodes(cli: &PrintOpcodesArgs) -> ExitCode {
    let mut builder = TsClassesBuilder::new_with_options(TsClassesBuilderOptions {
        print_code: cli.print_js_code,
    });

    for full_path in cli.ts_files.iter() {
        if let Err(e) = builder.add_files(&full_path) {
            eprintln!("Error parsing {}: {}", full_path.display(), e);
            return ExitCode::FAILURE;
        }
    }

    let classes = match builder.finalize() {
        Ok(classes) => classes,
        Err(e) => {
            eprintln!("Error building classes. {}", e);
            return ExitCode::FAILURE;
        }
    };

    let composite_opcodes = if cli.only_ts {
        SnythesisTask::get_classes_opcodes(&classes, true)
    } else {
        SnythesisTask::get_composite_opcodes(
            &classes,
            cli.max_sequence_size,
            !cli.ignore_string_ops,
        )
    };
    let opcodes_len = composite_opcodes.len();

    let sorted_opcodes = sort_opcodes(composite_opcodes);

    println!("Composite opcodes:");
    sorted_opcodes.composite_opcodes().for_each(|(_, ops)| {
        ops.iter().for_each(|op| println!("{}", op.op_name()));
    });

    if cli.print_summary {
        println!();
        println!("summary:");
        sorted_opcodes.composite_opcodes().for_each(|(arg_types, ops)| {
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
