//! Session artifact schemas and paths (task-level contract: metadata.json,
//! result.json, health.json under `<workspace>/.dv/`), all written with
//! temp-file + atomic-rename semantics.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::util::atomic_write;

/// Protocol major version; JSON keys are stable within a major version.
pub const PROTOCOL_VERSION: u32 = 1;
/// Filtered result output cap (bytes).
pub const RESULT_CAP: usize = 1_048_576;
/// Explicit truncation marker required by the spec.
pub const TRUNCATION_MARKER: &str = "[output truncated at 1048576 bytes]";

/// Session root directory name inside the workspace.
pub const SESSION_DIR: &str = ".dv";

#[derive(Clone, Debug)]
pub struct SessionPaths {
    pub root: PathBuf,
}

impl SessionPaths {
    pub fn new(workspace: &Path) -> Self {
        Self { root: workspace.join(SESSION_DIR) }
    }

    pub fn metadata(&self) -> PathBuf {
        self.root.join("metadata.json")
    }
    pub fn result(&self) -> PathBuf {
        self.root.join("result.json")
    }
    pub fn health(&self) -> PathBuf {
        self.root.join("health.json")
    }
    pub fn socket(&self) -> PathBuf {
        self.root.join("control.sock")
    }
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn published_cli(&self) -> PathBuf {
        self.bin_dir().join("dv")
    }
    pub fn raw_log(&self, stamp: &str) -> PathBuf {
        self.root.join(format!("session-{stamp}.log"))
    }
    /// Latest raw log by mtime, if any.
    pub fn latest_raw_log(&self) -> Option<PathBuf> {
        let mut logs: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&self.root)
            .ok()?
            .flatten()
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with("session-") && n.ends_with(".log")
            })
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        logs.sort();
        logs.pop().map(|(_, p)| p)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Starting,
    Idle,
    Busy,
    Crashed,
    Stopped,
    Unreachable,
    Unknown,
}

/// `metadata.json` — session identity and lifecycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub protocol_version: u32,
    pub tool_version: String,
    pub session_id: String,
    pub mode: String, // "headless" | "gui"
    pub state: SessionState,
    pub workspace: String,
    #[serde(default)]
    pub project: Option<String>,
    pub started_at: String,
    #[serde(default)]
    pub current_command: Option<String>,
    #[serde(default)]
    pub current_command_started_at: Option<String>,
    #[serde(default)]
    pub current_tool_operation: Option<ToolOperation>,
    #[serde(default)]
    pub last_tool_operation: Option<ToolOperation>,
    #[serde(default)]
    pub supervisor_pid: Option<i32>,
    #[serde(default)]
    pub vivado_pid: Option<i32>,
    #[serde(default)]
    pub socket_path: Option<String>,
    #[serde(default)]
    pub raw_log: Option<String>,
}

/// A managed `run <tool>` operation record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolOperation {
    pub tool: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub started_at: String,
    #[serde(default)]
    pub finished_at: Option<String>,
    pub state: String, // "running" | "completed" | "failed"
    #[serde(default)]
    pub exit_status: Option<i32>,
}

/// `result.json` — latched latest completed TCL result, or a marker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandResult {
    /// False for the durable `no completed command` marker.
    pub completed: bool,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub had_errors: bool,
    pub truncated: bool,
}

impl CommandResult {
    /// The durable marker written at dispatch so stale output can never leak.
    pub fn no_completed_command() -> Self {
        Self {
            completed: false,
            command: None,
            started_at: None,
            finished_at: None,
            output: None,
            errors: vec![],
            had_errors: false,
            truncated: false,
        }
    }
}

/// `health.json` — latest process-tree sample.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthSample {
    pub sampled_at: String,
    pub descendants: u32,
    pub cpu_percent: f64,
    pub rss_kib: u64,
    #[serde(default)]
    pub last_pty_read_at: Option<String>,
    pub last_pty_read_age_seconds: f64,
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut data = serde_json::to_vec_pretty(value)?;
    data.push(b'\n');
    atomic_write(path, &data)
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> std::io::Result<T> {
    let data = std::fs::read(path)?;
    serde_json::from_slice(&data).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_roundtrip_and_marker() {
        let dir = std::env::temp_dir().join(format!("davit-art-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("result.json");
        write_json(&p, &CommandResult::no_completed_command()).unwrap();
        let r: CommandResult = read_json(&p).unwrap();
        assert!(!r.completed && !r.had_errors && r.output.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_paths() {
        let sp = SessionPaths::new(Path::new("/workspace"));
        assert_eq!(sp.socket(), PathBuf::from("/workspace/.dv/control.sock"));
        assert_eq!(sp.published_cli(), PathBuf::from("/workspace/.dv/bin/dv"));
    }
}
