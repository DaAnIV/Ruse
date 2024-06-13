use std::{fs, path::{Path, PathBuf}, time::Duration};

use serde_with::{serde_as, DurationSeconds};

#[serde_as]
#[derive(serde::Deserialize)]
pub struct BenchmarkConfig {
    pub benchmarks: Vec<PathBuf>,
    pub output: PathBuf,
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub max_iterations: u32,
    pub print_inserted_programs: bool,
}

fn workspace_dir() -> PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}


impl Default for BenchmarkConfig {
    fn default() -> Self {
        let mut bencmarks_dir = workspace_dir();
        bencmarks_dir.push("benchmarks");
        bencmarks_dir.push("tasks");
        let mut output_dir = workspace_dir();
        output_dir.push("benchmarks_output");
        Self {  
            benchmarks: vec![bencmarks_dir],
            output: output_dir,
            timeout: Duration::from_secs(300),
            max_iterations: 5,
            print_inserted_programs: false
        }
    }
}

impl From<PathBuf> for BenchmarkConfig {
    fn from(path: PathBuf) -> Self {
        let config_data = fs::File::open(path).expect("Failed to open config file");
        let deserialized: Self = serde_json::from_reader(config_data).expect("Config file is corrupt");
        deserialized
    }
}

impl std::fmt::Display for BenchmarkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Timeout {:.3} seconds", self.timeout.as_secs_f32())?;
        writeln!(f, "Max iterations {}", self.max_iterations)?;

        Ok(())
    }
}