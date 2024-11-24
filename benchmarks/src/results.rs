use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
    vec,
};

use ruse_synthesizer::{prog::SubProgram, synthesizer::CurrentStatistics};

use serde::{Serialize, Serializer};
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarksIteration {
    time: Duration,
    statistics: CurrentStatistics,
}

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarkResult {
    path: PathBuf,
    iterations: Vec<BenchmarksIteration>,
    #[serde(serialize_with = "serialize_found")]
    found: Option<Arc<SubProgram>>,
    total_time: Option<Duration>,
    total_statistics: Option<CurrentStatistics>,
    error: Option<String>,
}

fn serialize_found<S>(found: &Option<Arc<SubProgram>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match found {
        Some(p) => serializer.serialize_str(&p.get_code()),
        None => serializer.serialize_str("-"),
    }
}

impl BenchmarkResult {
    pub fn new(path: &Path) -> Self {
        Self {
            path: PathBuf::from(path),
            iterations: vec![],
            found: None,
            total_time: None,
            total_statistics: None,
            error: None,
        }
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
        self.found = found;
        self.total_statistics = Some(statistics);
    }
}

#[derive(Serialize)]
struct Sysinfo {
    name: String,
    kernel: String,
    os: String,
    ram: u64,
    cpu_count: usize,
    cpu: Vec<String>,
    cpu_fq: Vec<u64>,
}

impl Sysinfo {
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
        Self {
            name: sysinfo::System::name().unwrap(),
            kernel: sysinfo::System::kernel_version().unwrap(),
            os: sysinfo::System::os_version().unwrap(),
            ram: sys.total_memory(),
            cpu_count: sys.cpus().len(),
            cpu: sys.cpus().iter().map(|cpu| cpu.name().to_owned()).collect(),
            cpu_fq: sys.cpus().iter().map(|cpu| cpu.frequency()).collect(),
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
    fn from_path_with_formatter(path: &Path, formatter: F) -> Self {
        let writer = File::create(path).expect("Failed to open output file");
        let mut this = Self {
            state: vec![],
            writer,
            formatter: formatter,
        };

        this.begin();
        this.serialize_entry("timestamp", &chrono::Utc::now().timestamp());
        this.serialize_entry("sysinfo", &Sysinfo::new());
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
    pub fn from_path(path: &Path) -> Self {
        Self::from_path_with_formatter(path, CompactFormatter)
    }
}

impl<'a> ResultsWriter<PrettyFormatter<'a>> {
    pub fn from_path_pretty(path: &Path) -> Self {
        Self::from_path_with_formatter(path, PrettyFormatter::new())
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
