use std::{fs::File, io::Write, path::{Path, PathBuf}, sync::Arc, time::Duration, vec};

use ruse_synthesizer::{prog::SubProgram, synthesizer::CurrentStatistics};

use serde::{Serialize, Serializer};

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarksIteration {
    time: Duration,
    statistics: CurrentStatistics,
}

#[derive(Serialize, Debug, Clone)]
pub struct BenchmarkResult {
    name: String,
    iterations: Vec<BenchmarksIteration>,
    #[serde(serialize_with = "serialize_found")]
    found: Option<Arc<SubProgram>>,
    total_time: Option<Duration>,
    total_statistics: Option<CurrentStatistics>,
    error: Option<String>
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
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            iterations: vec![],
            found: None,
            total_time: None,
            total_statistics: None,
            error: None
        }
    }

    pub fn add_iteration(&mut self, time: Duration, statistics: CurrentStatistics) {
        let iter_stats = match self.iterations.last() {
            Some(prev) => statistics.get_diff(&prev.statistics),
            None => statistics
        };
        self.iterations.push(BenchmarksIteration {
            time: time,
            statistics: iter_stats,
        });
    }

    pub fn error<E: std::error::Error>(
        &mut self,
        error: &E,
    ) {
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
    cpu: String,
    cpu_count: usize,
    cpu_fq: u64,
}

impl Sysinfo {
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu();
        sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
        Self {
            name: sysinfo::System::name().unwrap(),
            kernel: sysinfo::System::kernel_version().unwrap(),
            os: sysinfo::System::os_version().unwrap(),
            ram: sys.total_memory(),
            cpu: sys.global_cpu_info().name().to_owned(),
            cpu_count: sys.cpus().len(),
            cpu_fq: sys.global_cpu_info().frequency()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    First,
    Rest,    
}

pub(crate) struct ResultsWriter {
    path: PathBuf,
    writer: File,
    state: Vec<State>,
}

impl ResultsWriter {
    pub fn from_path(path: PathBuf) -> Self {
        let writer = File::create_new(path.clone()).expect("Failed to open output file");
        let mut this = Self {
            state: vec![State::First],
            writer: writer,
            path: path
        };

        this.begin();
        this.serialize_entry("timestamp", &chrono::Utc::now().timestamp());
        this.serialize_entry("sysinfo", &Sysinfo::new());
        this.begin_array("tasks");
        
        this
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn write_result(&mut self, result: &BenchmarkResult) {
        self.serialize_element(result);
    }

    fn begin(&mut self) {
        write!(self.writer, "{{").expect("Failed");
    }

    fn serialize_entry<T: Serialize>(&mut self, key: &str, value: &T) {
        self.serialize_dividor();
        self.serialize_key(key);
        self.serialize_value(value)
    }

    fn serialize_element<T: Serialize>(&mut self, value: &T) {
        self.serialize_dividor();
        self.serialize_value(value)
    }

    fn begin_array(&mut self, key: &str) {
        self.serialize_dividor();
        self.serialize_key(key);
        write!(self.writer, "[").expect("Failed");
        self.state.push(State::First);
    }

    fn serialize_dividor(&mut self) {
        if self.state.last().unwrap() == &State::First {
            self.state.pop();
            self.state.push(State::Rest);
        } else {
            write!(self.writer, ", ").expect("Failed");
        }
    }

    fn serialize_key(&mut self, key: &str) {
        write!(self.writer, "\"{}\": ", key).expect("Failed");
    }

    fn serialize_value<T: Serialize>(&mut self, value: &T) {
        serde_json::to_writer(&mut self.writer, value).expect("Failed");
    }
    
    fn end_array(&mut self) {
        self.state.pop();
        write!(self.writer, "]").expect("Failed");
    }

    fn end(&mut self) {
        self.end_array();
        write!(self.writer, "}}").expect("Failed");
    }
}

impl Drop for ResultsWriter {
    fn drop(&mut self) {
        self.end()
    }
}
