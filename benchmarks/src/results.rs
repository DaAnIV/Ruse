use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
    vec,
};

use byte_unit::{Byte, AdjustedByte, UnitType};
use ruse_synthesizer::{prog::SubProgram, synthesizer::CurrentStatistics};

use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};

use crate::config::BenchmarkConfig;

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarksIteration {
    time: Duration,
    statistics: CurrentStatistics,
}

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarkResult {
    path: PathBuf,
    pub opcode_count: usize,
    string_literals: Option<Vec<String>>,
    num_literals: Option<Vec<i64>>,
    iterations: Vec<BenchmarksIteration>,
    found: String,
    total_time: Option<Duration>,
    total_statistics: Option<CurrentStatistics>,
    max_vm_usage: Option<AdjustedByte>,
    error: Option<String>,
}

impl BenchmarkResult {
    pub fn new(path: &Path) -> Self {
        Self {
            path: PathBuf::from(path),
            opcode_count: 0,
            iterations: vec![],
            string_literals: None,
            num_literals: None,
            found: "_".to_owned(),
            total_time: None,
            total_statistics: None,
            max_vm_usage: None,
            error: None,
        }
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
    }

    pub fn error<E: std::error::Error>(&mut self, error: &E) {
        self.error = Some(error.to_string());
    }

    pub fn finish(
        &mut self,
        found: Option<Arc<SubProgram>>,
        time: Duration,
        statistics: CurrentStatistics,
    ) {
        self.total_time = Some(time);
        if let Some(p) = found {
            self.found = p.get_code();
        }
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

#[derive(Debug, PartialEq, Eq)]
enum State {
    First,
    Rest,
    FirstArray,
    RestArray,
}

impl State {
    fn is_arr(&self) -> bool {
        match self {
            State::FirstArray => true,
            State::RestArray => true,
            _ => false,
        }
    }

    fn is_first(&self) -> bool {
        match self {
            State::First => true,
            State::FirstArray => true,
            _ => false,
        }
    }
}

pub(crate) struct ResultsWriter<F>
where
    F: Formatter + Clone,
{
    writer: File,
    state: Vec<State>,
    formatter: F,
}

impl<F> ResultsWriter<F>
where
    F: Formatter + Clone,
{
    fn from_path_with_formatter(path: &Path, config: &BenchmarkConfig, formatter: F) -> Self {
        let writer = File::create(path).expect("Failed to open output file");
        let mut this = Self {
            state: vec![],
            writer,
            formatter: formatter,
        };

        this.begin();
        this.serialize_entry("timestamp", &chrono::Utc::now().timestamp());
        this.serialize_entry("sysinfo", &Sysinfo::new());
        this.serialize_entry("config", config);
        this.begin_array("tasks");

        this
    }

    pub fn write_result(&mut self, result: &BenchmarkResult) {
        self.serialize_element(result);
    }

    fn begin(&mut self) {
        self.state.push(State::First);
        self.formatter
            .begin_object(&mut self.writer)
            .expect("Failed");
    }

    fn serialize_entry<T: Serialize>(&mut self, key: &str, value: &T) {
        self.serialize_key(key);
        self.serialize_value(value)
    }

    fn serialize_element<T: Serialize>(&mut self, value: &T) {
        self.serialize_value(value)
    }

    fn begin_array(&mut self, key: &str) {
        self.serialize_key(key);
        self.formatter
            .begin_object_value(&mut self.writer)
            .expect("Failed");
        self.formatter
            .begin_array(&mut self.writer)
            .expect("Failed");
        self.state.push(State::FirstArray);
    }

    fn serialize_key(&mut self, key: &str) {
        let state = self.state.last_mut().unwrap();

        self.formatter
            .begin_object_key(&mut self.writer, state.is_first())
            .expect("Failed");
        *state = State::Rest;

        self.formatter
            .begin_string(&mut self.writer)
            .expect("Failed");
        self.formatter
            .write_string_fragment(&mut self.writer, key)
            .expect("Failed");
        self.formatter.end_string(&mut self.writer).expect("Failed");
        self.formatter
            .end_object_key(&mut self.writer)
            .expect("Failed");
    }

    fn serialize_value<T: Serialize>(&mut self, value: &T) {
        self.begin_value();

        let mut ser =
            serde_json::Serializer::with_formatter(&mut self.writer, self.formatter.clone());
        value.serialize(&mut ser).expect("Failed");

        self.end_value();
    }

    fn begin_value(&mut self) {
        let state = self.state.last_mut().unwrap();
        if state.is_arr() {
            self.formatter
                .begin_array_value(&mut self.writer, state.is_first())
                .expect("Failed");
            *state = State::RestArray;
        } else {
            self.formatter
                .begin_object_value(&mut self.writer)
                .expect("Failed");
        }
    }

    fn end_value(&mut self) {
        let state = self.state.last().unwrap();
        if state.is_arr() {
            self.formatter
                .end_array_value(&mut self.writer)
                .expect("Failed");
        } else {
            self.formatter
                .end_object_value(&mut self.writer)
                .expect("Failed");
        }
    }

    fn end_array(&mut self) {
        self.state.pop();
        self.formatter.end_array(&mut self.writer).expect("Failed");
    }

    fn end(&mut self) {
        self.end_array();
        self.formatter.end_object(&mut self.writer).expect("Failed");
    }
}

impl ResultsWriter<CompactFormatter> {
    pub fn from_path(path: &Path, config: &BenchmarkConfig) -> Self {
        Self::from_path_with_formatter(path, config, CompactFormatter)
    }
}

impl<'a> ResultsWriter<PrettyFormatter<'a>> {
    pub fn from_path_pretty(path: &Path, config: &BenchmarkConfig) -> Self {
        Self::from_path_with_formatter(path, config, PrettyFormatter::new())
    }
}

impl<F> Drop for ResultsWriter<F>
where
    F: Formatter + Clone,
{
    fn drop(&mut self) {
        self.end()
    }
}
