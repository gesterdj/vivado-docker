//! Client-side verbs: exec, stop, show, logs, diagnose, run.
//!
//! Everything here operates on the shared workspace only — the control
//! socket for exec/stop/run, plain artifact files (plus procfs) for
//! show/logs/diagnose. No container runtime is required.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::artifacts::*;
use crate::protocol::{read_frame, write_frame, Request, Response, MAX_FRAME};
use crate::util::human_duration;

/// Exit codes per the CLI spec.
pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_TIMEOUT: i32 = 3;

pub fn usage_err(msg: &str) -> ! {
    eprintln!("dv: {msg}");
    std::process::exit(EXIT_USAGE);
}

fn err(msg: &str) -> ! {
    eprintln!("dv: {msg}");
    std::process::exit(EXIT_ERROR);
}

/// Locate the workspace containing `.dv/`: `$DV_WORKSPACE`, else walk up
/// from the current directory, else `/workspace` (container default).
pub fn find_workspace() -> Option<PathBuf> {
    if let Ok(w) = std::env::var("DV_WORKSPACE") {
        let p = PathBuf::from(w);
        return if p.join(SESSION_DIR).is_dir() { Some(p) } else { None };
    }
    if let Ok(mut cur) = std::env::current_dir() {
        loop {
            if cur.join(SESSION_DIR).is_dir() {
                return Some(cur);
            }
            if !cur.pop() {
                break;
            }
        }
    }
    let fallback = PathBuf::from("/workspace");
    if fallback.join(SESSION_DIR).is_dir() {
        return Some(fallback);
    }
    None
}

fn require_session() -> SessionPaths {
    match find_workspace() {
        Some(ws) => SessionPaths::new(&ws),
        None => usage_err("no session found (no .dv/ in this or any parent directory); run `dv start` first"),
    }
}

fn connect(paths: &SessionPaths) -> std::io::Result<UnixStream> {
    UnixStream::connect(paths.socket())
}

/// Effective session state combining metadata with liveness evidence:
/// a live-looking metadata whose daemon shows no sign of life is
/// `unreachable`. Read-only: never rewrites artifacts.
///
/// Liveness must work across PID namespaces (host, session container,
/// sidecars all read the same .dv/). Two signals, either suffices:
/// - the supervisor PID exists here AND its comm is `dv` (same
///   namespace; the comm check rejects PID collisions), or
/// - the health.json heartbeat (rewritten every ~10 s from readiness
///   onward) is fresh.
pub fn effective_state(paths: &SessionPaths) -> (Option<Metadata>, SessionState) {
    let meta: Option<Metadata> = read_json(&paths.metadata()).ok();
    let Some(m) = &meta else {
        return (None, SessionState::Unknown);
    };
    match m.state {
        SessionState::Stopped | SessionState::Crashed => (meta.clone(), m.state),
        st => {
            let pid_alive = m.supervisor_pid.map(daemon_pid_alive).unwrap_or(false);
            if pid_alive || heartbeat_fresh(paths) {
                (meta.clone(), st)
            } else {
                (meta.clone(), SessionState::Unreachable)
            }
        }
    }
}

/// PID exists in this namespace and really is a dv daemon.
fn daemon_pid_alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|c| c.trim() == "dv")
        .unwrap_or(false)
}

/// Heartbeat is fresh when health.json's mtime is within three sampler
/// periods (sampler writes every ~10 s).
fn heartbeat_fresh(paths: &SessionPaths) -> bool {
    std::fs::metadata(paths.health())
        .and_then(|md| md.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age.as_secs() < 30)
        .unwrap_or(false)
}

// ---------------------------------------------------------------- exec

pub struct ExecArgs {
    pub timeout: Option<u64>,
    pub file: Option<String>,
    pub inline: Vec<String>,
}

pub fn exec(args: ExecArgs) -> i32 {
    let paths = require_session();
    let tcl = match (&args.file, args.inline.is_empty()) {
        (Some(_), false) => usage_err("supply either inline TCL or --file, not both"),
        (None, true) => usage_err("supply a TCL command or --file TCL_FILE"),
        (Some(f), true) => match std::fs::read_to_string(f) {
            Ok(t) => t.trim_end().to_string(),
            Err(e) => usage_err(&format!("--file {f}: {e}")),
        },
        // Inline args are joined with single spaces after argv parsing.
        (None, false) => args.inline.join(" "),
    };

    let mut stream = match connect(&paths) {
        Ok(s) => s,
        Err(e) => report_unreachable(&paths, &e),
    };
    if write_frame(&mut stream, &Request::Exec { tcl }).is_err() {
        err("failed to send command to the session daemon");
    }
    if let Some(t) = args.timeout {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(t)));
    }
    match read_frame::<_, Response>(&mut stream, MAX_FRAME) {
        Ok(Response::Result { result, .. }) => print_result(&result),
        Ok(Response::Busy { current_command, dispatched_at, last_pty_read_at, .. }) => {
            eprintln!(
                "dv: session busy: `{current_command}` (dispatched {dispatched_at}, last PTY read {})",
                last_pty_read_at.as_deref().unwrap_or("never")
            );
            EXIT_ERROR
        }
        Ok(Response::Error { code, message, .. }) => {
            if code == "crashed" {
                eprintln!("dv: Vivado process has died (not a TCL error): {message}");
            } else {
                eprintln!("dv: {code}: {message}");
            }
            if code == "usage" {
                EXIT_USAGE
            } else {
                EXIT_ERROR
            }
        }
        Ok(_) => err("unexpected response from daemon"),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            // Client-side timeout only: the daemon runs the command to
            // completion; the result will be latched.
            eprintln!(
                "dv: client wait timed out after {}s; the command continues in the session. \
                 Retrieve the outcome with `dv show result`.",
                args.timeout.unwrap_or(0)
            );
            EXIT_TIMEOUT
        }
        Err(e) => err(&format!("protocol failure: {e}")),
    }
}

fn print_result(result: &serde_json::Value) -> i32 {
    if let Some(out) = result.get("output").and_then(|v| v.as_str()) {
        if !out.is_empty() {
            println!("{out}");
        }
    }
    let had_errors = result.get("had_errors").and_then(|v| v.as_bool()).unwrap_or(false);
    if had_errors {
        for e in result.get("errors").and_then(|v| v.as_array()).into_iter().flatten() {
            if let Some(s) = e.as_str() {
                eprintln!("{s}");
            }
        }
        EXIT_ERROR
    } else {
        EXIT_OK
    }
}

fn report_unreachable(paths: &SessionPaths, e: &std::io::Error) -> ! {
    let (_, st) = effective_state(paths);
    match st {
        SessionState::Unknown => usage_err("no session metadata found; run `dv start` first"),
        SessionState::Stopped => err("session is stopped; run `dv start`"),
        SessionState::Crashed => err("session crashed; inspect with `dv diagnose inspect` and restart"),
        _ => err(&format!(
            "session control socket unreachable ({e}); state looks {st:?}. \
             If the container is gone, use `dv stop --force` from the host."
        )),
    }
}

// ---------------------------------------------------------------- stop

pub fn stop(force: bool) -> i32 {
    if force {
        usage_err(
            "`stop --force` needs the container runtime and is handled by the host \
             launcher; from a sidecar, lifecycle is owned by the orchestrator",
        );
    }
    let paths = require_session();
    let mut stream = match connect(&paths) {
        Ok(s) => s,
        Err(e) => report_unreachable(&paths, &e),
    };
    if write_frame(&mut stream, &Request::Stop).is_err() {
        err("failed to send stop request");
    }
    match read_frame::<_, Response>(&mut stream, MAX_FRAME) {
        Ok(Response::Ok { message, .. }) => {
            eprintln!("dv: {message}");
            EXIT_OK
        }
        Ok(Response::Busy { current_command, dispatched_at, .. }) => {
            let elapsed = elapsed_since(&dispatched_at);
            eprintln!(
                "dv: refusing to stop: `{current_command}` is running ({elapsed}). \
                 Use `dv stop --force` from the host to escalate (in-flight result may be lost)."
            );
            EXIT_ERROR
        }
        _ => err("unexpected response from daemon"),
    }
}

fn elapsed_since(iso: &str) -> String {
    // Best-effort parse of our own ISO format for a human elapsed time.
    parse_iso_secs(iso)
        .map(|start| {
            let now = crate::util::unix_now();
            human_duration(now.saturating_sub(start))
        })
        .unwrap_or_else(|| "unknown elapsed".into())
}

fn parse_iso_secs(iso: &str) -> Option<u64> {
    // "YYYY-MM-DDTHH:MM:SSZ"
    let b = iso.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| iso.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // days from civil (inverse of util::civil_from_unix)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86400 + h * 3600 + mi * 60 + s) as u64)
}

// ---------------------------------------------------------------- show

pub fn show(object: &str, json: bool) -> i32 {
    let paths = require_session();
    match object {
        "status" => show_status(&paths, json),
        "result" => show_result(&paths, json),
        "metadata" => dump_json_file(&paths.metadata(), json, "no session metadata"),
        "health" => match read_json::<HealthSample>(&paths.health()) {
            Ok(h) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&h).unwrap());
                } else {
                    println!(
                        "sampled_at: {}\ndescendants: {}\ncpu_percent: {}\nrss_kib: {}\nlast_pty_read_age_seconds: {}",
                        h.sampled_at, h.descendants, h.cpu_percent, h.rss_kib, h.last_pty_read_age_seconds
                    );
                }
                EXIT_OK
            }
            Err(_) => {
                eprintln!("dv: no health sample exists yet");
                EXIT_USAGE
            }
        },
        _ => usage_err("show takes one of: status, result, metadata, health"),
    }
}

fn show_status(paths: &SessionPaths, json: bool) -> i32 {
    let (meta, state) = effective_state(paths);
    let Some(m) = meta else {
        eprintln!("dv: no session metadata found");
        return EXIT_USAGE;
    };
    let health: Option<HealthSample> = read_json(&paths.health()).ok();
    if json {
        let v = serde_json::json!({
            "mode": m.mode,
            "state": state,
            "current_command": m.current_command,
            "current_command_started_at": m.current_command_started_at,
            "current_tool_operation": m.current_tool_operation,
            "last_tool_operation": m.last_tool_operation,
            "project": m.project,
            "started_at": m.started_at,
            "health": health,
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    } else {
        println!("mode: {}", m.mode);
        println!("state: {}", serde_json::to_value(state).unwrap().as_str().unwrap_or("?"));
        if let Some(c) = &m.current_command {
            let since = m.current_command_started_at.as_deref().unwrap_or("?");
            println!("current command: {c} (since {since})");
        }
        if let Some(h) = &health {
            println!(
                "health: {} descendants, {:.1}% cpu, {} KiB rss, {:.1}s since last PTY read",
                h.descendants, h.cpu_percent, h.rss_kib, h.last_pty_read_age_seconds
            );
        }
        println!("started: {}", m.started_at);
    }
    EXIT_OK
}

fn show_result(paths: &SessionPaths, json: bool) -> i32 {
    match read_json::<CommandResult>(&paths.result()) {
        Ok(r) if r.completed => {
            if json {
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
                if r.had_errors {
                    EXIT_ERROR
                } else {
                    EXIT_OK
                }
            } else {
                print_result(&serde_json::to_value(&r).unwrap())
            }
        }
        Ok(_) => {
            eprintln!("dv: no completed command result exists");
            EXIT_USAGE
        }
        Err(_) => {
            eprintln!("dv: no completed command result exists");
            EXIT_USAGE
        }
    }
}

fn dump_json_file(path: &Path, _json: bool, missing: &str) -> i32 {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            print!("{s}");
            EXIT_OK
        }
        Err(_) => {
            eprintln!("dv: {missing}");
            EXIT_USAGE
        }
    }
}

// ---------------------------------------------------------------- logs

pub fn logs(tail: usize, follow: bool) -> i32 {
    let paths = require_session();
    let Some(log) = paths.latest_raw_log() else {
        eprintln!("dv: no session log exists");
        return EXIT_USAGE;
    };
    // Reads the artifact directly — never TCL, never the socket.
    let content = std::fs::read_to_string(&log).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(tail);
    for l in &lines[start..] {
        println!("{l}");
    }
    if follow {
        let mut pos = content.len() as u64;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let Ok(mut f) = std::fs::File::open(&log) else { break };
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            if len > pos {
                use std::io::Seek;
                let _ = f.seek(std::io::SeekFrom::Start(pos));
                let mut buf = String::new();
                let _ = f.read_to_string(&mut buf);
                print!("{buf}");
                let _ = std::io::stdout().flush();
                pos = len;
            }
        }
    }
    EXIT_OK
}

// ------------------------------------------------------------- diagnose

pub fn diagnose(probe: &str, tail: usize, json: bool) -> i32 {
    let paths = require_session();
    // Every probe is read-only: session artifacts, runtime inspection,
    // and procfs only. Never the socket, never the PTY, never TCL.
    match probe {
        "last" => show_result(&paths, json),
        "metadata" => dump_json_file(&paths.metadata(), json, "no session metadata"),
        "health" => show(&"health".to_string(), json_flag(json)),
        "logs" => logs_tail_only(&paths, tail),
        "inspect" => inspect(&paths, json),
        "ps" => {
            let meta: Option<Metadata> = read_json(&paths.metadata()).ok();
            let root = meta.and_then(|m| m.vivado_pid).unwrap_or(-1);
            let entries = crate::procfs::ps_tree(root, 1000);
            if json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else {
                println!("{:>8} {:>2} {:>7} {:>10} COMM", "PID", "S", "CPU%", "RSS_KIB");
                for e in entries {
                    println!("{:>8} {:>2} {:>7.1} {:>10} {}", e.pid, e.state, e.cpu_percent, e.rss_kib, e.comm);
                }
            }
            EXIT_OK
        }
        "wchan" => {
            let meta: Option<Metadata> = read_json(&paths.metadata()).ok();
            let pids: Vec<i32> = meta
                .iter()
                .flat_map(|m| [m.supervisor_pid, m.vivado_pid])
                .flatten()
                .collect();
            let entries = crate::procfs::wchan(&pids);
            if json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else {
                for e in entries {
                    println!("{:>8} {:<16} {}", e.pid, e.comm, e.wchan);
                }
            }
            EXIT_OK
        }
        "fdtable" => {
            let meta: Option<Metadata> = read_json(&paths.metadata()).ok();
            let pid = meta.and_then(|m| m.supervisor_pid).unwrap_or(-1);
            let entries = crate::procfs::fdtable(pid);
            if json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else {
                for e in entries {
                    println!("{:>4} {}", e.fd, e.target);
                }
            }
            EXIT_OK
        }
        "fionread" => fionread_probe(&paths, json),
        _ => usage_err(
            "diagnose takes one of: last, metadata, health, inspect, logs, ps, wchan, fionread, fdtable",
        ),
    }
}

fn json_flag(json: bool) -> bool {
    json
}

fn logs_tail_only(paths: &SessionPaths, tail: usize) -> i32 {
    let Some(log) = paths.latest_raw_log() else {
        eprintln!("dv: no session log exists");
        return EXIT_USAGE;
    };
    let content = std::fs::read_to_string(&log).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(tail);
    for l in &lines[start..] {
        println!("{l}");
    }
    EXIT_OK
}

/// Composite inspect: metadata, health, result, 50-line log tail, sample
/// time. Missing optional data is `null`, never fabricated.
fn inspect(paths: &SessionPaths, json: bool) -> i32 {
    let meta: Option<serde_json::Value> = read_json(&paths.metadata()).ok();
    let health: Option<serde_json::Value> = read_json(&paths.health()).ok();
    let result: Option<serde_json::Value> = read_json(&paths.result()).ok();
    let log_tail: Option<String> = paths.latest_raw_log().and_then(|p| {
        let content = std::fs::read_to_string(p).ok()?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(50);
        Some(lines[start..].join("\n"))
    });
    let v = serde_json::json!({
        "sampled_at": crate::util::iso8601_now(),
        "metadata": meta,
        "health": health,
        "result": result,
        "log_tail": log_tail,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    }
    EXIT_OK
}

/// Non-destructive queued-byte counts for the supervisor's PTY fds,
/// obtained via pidfd_getfd so the ioctl targets the same open file
/// description (never a re-open, never a read).
fn fionread_probe(paths: &SessionPaths, json: bool) -> i32 {
    let meta: Option<Metadata> = read_json(&paths.metadata()).ok();
    let Some(pid) = meta.and_then(|m| m.supervisor_pid) else {
        eprintln!("dv: no supervisor pid in metadata");
        return EXIT_USAGE;
    };
    let mut entries = vec![];
    for fd in crate::procfs::fdtable(pid) {
        if fd.target.contains("/dev/ptmx") || fd.target.contains("/dev/pts/") {
            let queued = pidfd_fionread(pid, fd.fd);
            entries.push(serde_json::json!({
                "fd": fd.fd,
                "target": fd.target,
                "queued_bytes": queued.as_ref().ok(),
                "error": queued.err().map(|e| e.to_string()),
            }));
        }
    }
    let v = serde_json::Value::Array(entries);
    if json {
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    }
    EXIT_OK
}

fn pidfd_fionread(pid: i32, fd: i32) -> std::io::Result<i32> {
    unsafe {
        let pidfd = libc::syscall(libc::SYS_pidfd_open, pid, 0);
        if pidfd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let dup = libc::syscall(libc::SYS_pidfd_getfd, pidfd, fd, 0);
        let e = if dup < 0 { Some(std::io::Error::last_os_error()) } else { None };
        libc::close(pidfd as i32);
        if let Some(e) = e {
            return Err(e);
        }
        let mut n: libc::c_int = 0;
        let rc = libc::ioctl(dup as i32, libc::FIONREAD, &mut n);
        let e = if rc < 0 { Some(std::io::Error::last_os_error()) } else { None };
        libc::close(dup as i32);
        match e {
            Some(e) => Err(e),
            None => Ok(n),
        }
    }
}

// ---------------------------------------------------------------- run

pub fn run_tool(tool: &str, argv: Vec<String>) -> i32 {
    if !crate::daemon::TOOLS.contains(&tool) {
        usage_err("run takes one of: xsct, xsdb, bootgen, dtc");
    }
    if tool == "xsct" && argv.is_empty() {
        usage_err("run xsct requires a TCL file argument");
    }
    let paths = require_session();
    let mut stream = match connect(&paths) {
        Ok(s) => s,
        Err(e) => report_unreachable(&paths, &e),
    };
    if write_frame(&mut stream, &Request::RunTool { tool: tool.into(), argv }).is_err() {
        err("failed to send tool request");
    }
    // Stream frames until Exit; argv/stdio/exit status are preserved.
    loop {
        match read_frame::<_, Response>(&mut stream, MAX_FRAME) {
            Ok(Response::Stream { fd, data, .. }) => {
                if fd == 2 {
                    eprint!("{data}");
                    let _ = std::io::stderr().flush();
                } else {
                    print!("{data}");
                    let _ = std::io::stdout().flush();
                }
            }
            Ok(Response::Exit { exit_status, .. }) => return exit_status,
            Ok(Response::Busy { current_command, dispatched_at, .. }) => {
                eprintln!(
                    "dv: session busy: `{current_command}` (dispatched {dispatched_at}); tool operations share the workflow scheduler"
                );
                return EXIT_ERROR;
            }
            Ok(Response::Error { code, message, .. }) => {
                eprintln!("dv: {code}: {message}");
                return if code == "usage" { EXIT_USAGE } else { EXIT_ERROR };
            }
            Ok(_) => err("unexpected response from daemon"),
            Err(e) => err(&format!("protocol failure: {e}")),
        }
    }
}

// ---------------------------------------------------------- healthcheck

/// Hidden verb used by the image HEALTHCHECK: healthy only when the
/// session is ready (idle or busy) and the daemon is alive.
pub fn healthcheck() -> i32 {
    let Some(ws) = find_workspace() else { return 1 };
    let paths = SessionPaths::new(&ws);
    let (_, state) = effective_state(&paths);
    match state {
        SessionState::Idle | SessionState::Busy => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_parse_roundtrip() {
        let t = parse_iso_secs("2025-07-16T16:41:22Z").unwrap();
        assert_eq!(t, 1_752_684_082);
    }
}
