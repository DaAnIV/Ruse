use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
    vec,
};

use byte_unit::{AdjustedByte, Byte, UnitType};
use ruse_synthesizer::{prog::SubProgram, synthesizer::CurrentStatistics};

use ruse_task_parser::SnythesisTaskCategory;
use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};

use crate::config::BenchmarkConfig;

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarksIteration {
    time: Duration,
    statistics: CurrentStatistics,
}

#[derive(Serialize, Debug, Clone)]
struct ResultSolution {
    found: String,
    num_mutations: u32,
    solution_size: u32,
    solution_depth: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarkResult {
    path: PathBuf,
    source: Option<String>,
    category: Option<String>,
    pub opcode_count: usize,
    string_literals: Option<Vec<String>>,
    num_literals: Option<Vec<i64>>,
    iterations: Vec<BenchmarksIteration>,
    iteration_count: usize,
    found: Option<ResultSolution>,
    total_time: Option<Duration>,
    total_statistics: Option<CurrentStatistics>,
    max_vm_usage: Option<AdjustedByte>,
    error: Option<String>,
}

impl BenchmarkResult {
    pub fn new(path: &Path) -> Self {
        Self {
            path: PathBuf::from(path),
            source: None,
            category: None,
            opcode_count: 0,
            iterations: vec![],
            iteration_count: 0,
            string_literals: None,
            num_literals: None,
            found: None,
            total_time: None,
            total_statistics: None,
            max_vm_usage: None,
            error: None,
        }
    }

    pub fn set_source(&mut self, source: String) {
        self.source = Some(source);
    }

    pub fn set_category(&mut self, category: SnythesisTaskCategory) {
        self.category = Some(category.to_string());
    }

    pub fn set_literals(&mut self, string_literals: Vec<String>, num_literals: Vec<i64>) {
        self.string_literals.replace(string_literals);
        self.num_literals.replace(num_literals);
    }

    pub fn add_iteration(&mut self, time: Duration, statistics: CurrentStatistics) {
        let iter_stats = match self.iterations.last() {
            Some(prev) => statistics.get_diff(&prev.statistics),
            None => statistics,
        };
        self.iterations.push(BenchmarksIteration {
            time,
            statistics: iter_stats,
        });
        self.iteration_count += 1;
    }

    pub fn error<E: std::error::Error>(&mut self, error: &E) {
        self.error = Some(error.to_string());
    }

    pub fn error_string(&mut self, error: &str) {
        self.error = Some(error.to_string());
    }

    pub fn set_total_time(&mut self, time: Duration) {
        self.total_time = Some(time);
    }

    pub fn set_found(&mut self, p: &SubProgram) {
        self.found = Some(ResultSolution {
            found: p.get_code(),
            num_mutations: p.num_mutations(),
            solution_size: p.size(),
            solution_depth: p.depth(),
        });
    }

    pub fn set_total_statistics(&mut self, statistics: CurrentStatistics) {
        self.total_statistics = Some(statistics);
    }

    pub(crate) fn set_max_vm_usage(&mut self, max_vm_usage: Byte) {
        self.max_vm_usage = Some(max_vm_usage.get_appropriate_unit(UnitType::Decimal));
    }
}

#[derive(Serialize)]
struct Sysinfo {
    name: String,
    kernel: String,
    os: String,
    ram: u64,
    cpu: String,
    cpu_fq: u64,
    cpu_core_count: usize,
}

impl Sysinfo {
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        let cpu_brand = sys.cpus()[0].brand().to_string();
        let cpu_fq =
            sys.cpus().iter().map(|x| x.frequency()).sum::<u64>() / sys.cpus().len() as u64;
        Self {
            name: sysinfo::System::name().unwrap(),
            kernel: sysinfo::System::kernel_version().unwrap(),
            os: sysinfo::System::os_version().unwrap(),
            ram: sys.total_memory(),
            cpu: cpu_brand,
            cpu_core_count: sys.cpus().len(),
            cpu_fq: cpu_fq,
        }
    }
}

#[derive(Serialize)]
struct Metadata<'a> {
    timestamp: i64,
    pid: u32,
    sysinfo: Sysinfo,
    config: &'a BenchmarkConfig,
}

#[derive(Clone)]
pub(crate) struct ResultsWriter<F>
where
    F: Formatter + Sync + Send + Clone + 'static,
{
    results_dir: PathBuf,
    formatter: F,
}

impl<F> ResultsWriter<F>
where
    F: Formatter + Sync + Send + Clone + 'static,
{
    fn from_path_with_formatter(path: &Path, config: &BenchmarkConfig, formatter: F) -> Self {
        let self_ = Self {
            results_dir: path.to_path_buf(),
            formatter: formatter,
        };

        std::fs::create_dir(path).expect("Failed to create output dir");

        let mut ser = self_.create_serializer("metadata.json");
        Metadata {
            timestamp: chrono::Utc::now().timestamp(),
            pid: std::process::id(),
            sysinfo: Sysinfo::new(),
            config: config,
        }
        .serialize(&mut ser)
        .expect("Failed to write metadata");

        self_
    }

    pub fn write_result(&self, result: &BenchmarkResult, i: usize) {
        let mut ser = self.create_serializer(&format!("task_{}.json", i));
        result.serialize(&mut ser).expect("Failed to write result");
    }

    fn create_serializer(&self, name: &str) -> serde_json::Serializer<File, F> {
        let file = std::fs::File::create(self.results_dir.join(name))
            .expect("Failed to create output file");
        serde_json::Serializer::with_formatter(file, self.formatter.clone())
    }
}

impl<'a> ResultsWriter<PrettyFormatter<'a>> {
    pub fn from_path_pretty(path: &Path, config: &BenchmarkConfig) -> Self {
        Self::from_path_with_formatter(path, config, PrettyFormatter::new())
    }
}
