//! The headless session daemon (`dv _daemon`).
//!
//! Owns Vivado through a PTY, serializes TCL and tool operations through
//! one scheduler, latches results atomically, filters output, samples
//! health, and serves the framed Unix-socket protocol. The daemon is the
//! container's foreground application: its exit ends the container.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::artifacts::*;
use crate::filter::{load_policy, Filter};
use crate::protocol::{read_frame, write_frame, Request, Response, MAX_REQUEST};
use crate::pty::{spawn_on_pty, strip_prompt, PromptMatcher, Pty};
use crate::util::{iso8601, iso8601_now, stamp_now, unix_now};

const UDEV_STUB: &str = "/opt/udev_stub.so";
pub const TOOLS: [&str; 4] = ["xsct", "xsdb", "bootgen", "dtc"];

struct Collector {
    /// Output buffer for the in-flight command (None = not collecting).
    buf: Option<Vec<u8>>,
    /// Set when the prompt returned for the in-flight command.
    done: bool,
    /// Set when Vivado exited (EOF on PTY).
    died: bool,
    matcher: PromptMatcher,
}

struct InFlight {
    command: String,
    dispatched_at: String,
}

pub struct Shared {
    paths: SessionPaths,
    meta: Mutex<Metadata>,
    inflight: Mutex<Option<InFlight>>,
    collector: Mutex<Collector>,
    cv: Condvar,
    pty_writer: Mutex<File>,
    raw_log: Mutex<File>,
    last_pty_read: Mutex<Option<SystemTime>>,
    vivado_pid: i32,
    shutdown: AtomicBool,
    workspace: PathBuf,
}

impl Shared {
    fn log_raw(&self, data: &[u8]) {
        let mut f = self.raw_log.lock().unwrap();
        let _ = f.write_all(data);
        let _ = f.flush();
    }

    fn log_line(&self, line: &str) {
        self.log_raw(format!("\n[davit {}] {}\n", iso8601_now(), line).as_bytes());
    }

    fn write_meta(&self) {
        let m = self.meta.lock().unwrap();
        let _ = write_json(&self.paths.metadata(), &*m);
    }

    fn set_state(&self, st: SessionState) {
        self.meta.lock().unwrap().state = st;
        self.write_meta();
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("dv daemon: {msg}");
    std::process::exit(1);
}

/// Entry point for `dv _daemon [--project REL.xpr]`.
pub fn run(workspace: &Path, project: Option<String>) -> ! {
    // Fail fast: never root, workspace must be writable (task 2.6).
    if unsafe { libc::geteuid() } == 0 {
        fail("refusing to run as root; start the container with -u UID:GID");
    }
    let paths = SessionPaths::new(workspace);
    fs::create_dir_all(paths.bin_dir())
        .unwrap_or_else(|e| fail(&format!("session root {} not writable: {e}", paths.root.display())));

    // Publish our own binary for sidecar callers.
    publish_self(&paths);

    // Validate project before starting Vivado.
    let project_abs = project.as_ref().map(|p| {
        let abs = if Path::new(p).is_absolute() { PathBuf::from(p) } else { workspace.join(p) };
        let canon = abs.canonicalize().unwrap_or_else(|e| fail(&format!("project {p}: {e}")));
        if !canon.starts_with(workspace.canonicalize().unwrap_or_else(|_| workspace.into())) {
            fail(&format!("project {p} lies outside the workspace"));
        }
        if canon.extension().and_then(|e| e.to_str()) != Some("xpr") {
            fail(&format!("project {p} does not have an .xpr suffix"));
        }
        canon
    });

    let stamp = stamp_now();
    let raw_log_path = paths.raw_log(&stamp);
    let raw_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&raw_log_path)
        .unwrap_or_else(|e| fail(&format!("cannot open raw log: {e}")));

    // Spawn Vivado on a PTY. DV_VIVADO_CMD overrides for testing.
    let vivado_cmd = std::env::var("DV_VIVADO_CMD").unwrap_or_else(|_| "vivado".into());
    let mut argv: Vec<String> = vec![vivado_cmd, "-mode".into(), "tcl".into()];
    if let Ok(extra) = std::env::var("DV_VIVADO_ARGS") {
        argv.extend(extra.split_whitespace().map(String::from));
    }
    // udev stub scoped to the Vivado process tree only (never the daemon
    // or ad-hoc shells).
    let mut envs: Vec<(String, String)> = vec![("HOME".into(), workspace.display().to_string())];
    if Path::new(UDEV_STUB).exists() {
        envs.push(("LD_PRELOAD".into(), UDEV_STUB.into()));
    }
    if let Ok(url) = std::env::var("VIVADO_HW_SERVER_URL") {
        envs.push(("VIVADO_HW_SERVER_URL".into(), url));
    }
    let pty: Pty = spawn_on_pty(&argv, &envs).unwrap_or_else(|e| fail(&format!("spawn vivado: {e}")));
    let vivado_pid = pty.child.id() as i32;
    let writer = pty.try_clone_master().unwrap_or_else(|e| fail(&format!("pty clone: {e}")));

    let session_id = format!("{}-{}", stamp, std::process::id());
    let meta = Metadata {
        protocol_version: PROTOCOL_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        session_id,
        mode: "headless".into(),
        state: SessionState::Starting,
        workspace: workspace.display().to_string(),
        project: project.clone(),
        started_at: iso8601_now(),
        current_command: None,
        current_command_started_at: None,
        current_tool_operation: None,
        last_tool_operation: None,
        supervisor_pid: Some(std::process::id() as i32),
        vivado_pid: Some(vivado_pid),
        socket_path: Some(paths.socket().display().to_string()),
        raw_log: Some(raw_log_path.display().to_string()),
    };

    let shared = Arc::new(Shared {
        paths: paths.clone(),
        meta: Mutex::new(meta),
        inflight: Mutex::new(None),
        collector: Mutex::new(Collector {
            buf: Some(Vec::new()), // collect startup output until first prompt
            done: false,
            died: false,
            matcher: PromptMatcher::new(),
        }),
        cv: Condvar::new(),
        pty_writer: Mutex::new(writer),
        raw_log: Mutex::new(raw_log),
        last_pty_read: Mutex::new(None),
        vivado_pid,
        shutdown: AtomicBool::new(false),
        workspace: workspace.to_path_buf(),
    });
    shared.write_meta();
    let _ = write_json(&paths.result(), &CommandResult::no_completed_command());

    // PTY reader thread: raw log + prompt matching, forever.
    spawn_reader(shared.clone(), pty);

    // Readiness: wait for the first prompt.
    shared.log_line("waiting for Vivado prompt");
    if !wait_prompt(&shared, Duration::from_secs(startup_timeout())) {
        shared.set_state(SessionState::Crashed);
        fail("Vivado did not reach its prompt during startup");
    }

    // Optional project open, gated before readiness.
    if let Some(pp) = &project_abs {
        let cmd = format!("open_project {{{}}}", pp.display());
        shared.log_line(&format!("opening project: {}", pp.display()));
        let result = execute_tcl(&shared, &cmd);
        if result.had_errors {
            shared.set_state(SessionState::Crashed);
            fail(&format!("project open failed: {:?}", result.errors));
        }
    }

    shared.set_state(SessionState::Idle);
    shared.log_line("session ready");

    // Health sampler (task 2.4).
    spawn_health_sampler(shared.clone());
    install_sigterm_handler();

    // Socket accept loop.
    let sock_path = paths.socket();
    let _ = fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)
        .unwrap_or_else(|e| fail(&format!("bind {}: {e}", sock_path.display())));
    let _ = fs::set_permissions(&sock_path, fs::Permissions::from_mode(0o600));
    listener.set_nonblocking(true).ok();

    loop {
        if SIGTERM.load(Ordering::SeqCst) || shared.shutdown.load(Ordering::SeqCst) {
            break;
        }
        if shared.collector.lock().unwrap().died {
            // Vivado death outside a command: coupled lifetime.
            shared.set_state(SessionState::Crashed);
            let _ = fs::remove_file(&sock_path);
            fail("Vivado process exited unexpectedly");
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let sh = shared.clone();
                std::thread::spawn(move || handle_connection(sh, stream));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }

    // Graceful shutdown: ask Vivado to exit, wait bounded.
    shared.log_line("shutting down");
    {
        let mut w = shared.pty_writer.lock().unwrap();
        let _ = w.write_all(b"exit\n");
        let _ = w.flush();
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if unsafe { libc::kill(shared.vivado_pid, 0) } != 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if unsafe { libc::kill(shared.vivado_pid, 0) } == 0 {
        unsafe { libc::kill(shared.vivado_pid, libc::SIGTERM) };
    }
    shared.set_state(SessionState::Stopped);
    let _ = fs::remove_file(&sock_path);
    std::process::exit(0);
}

fn startup_timeout() -> u64 {
    std::env::var("DV_STARTUP_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(600)
}

static SIGTERM: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigterm(_: libc::c_int) {
    SIGTERM.store(true, Ordering::SeqCst);
}

fn install_sigterm_handler() {
    unsafe {
        libc::signal(libc::SIGTERM, on_sigterm as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_sigterm as *const () as libc::sighandler_t);
    }
}

/// Copy our own executable to `.dv/bin/dv` (task 2.6).
fn publish_self(paths: &SessionPaths) {
    let me = std::env::current_exe().unwrap_or_else(|e| fail(&format!("current_exe: {e}")));
    let dst = paths.published_cli();
    let tmp = paths.bin_dir().join(".dv.tmp");
    fs::copy(&me, &tmp).unwrap_or_else(|e| fail(&format!("publish CLI: {e}")));
    let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    fs::rename(&tmp, &dst).unwrap_or_else(|e| fail(&format!("publish CLI: {e}")));
}

fn spawn_reader(shared: Arc<Shared>, mut pty: Pty) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match crate::pty::read_chunk(&mut pty.master, &mut buf) {
                Ok(0) | Err(_) => {
                    let _ = pty.child.wait();
                    let mut c = shared.collector.lock().unwrap();
                    c.died = true;
                    shared.cv.notify_all();
                    return;
                }
                Ok(n) => {
                    shared.log_raw(&buf[..n]);
                    *shared.last_pty_read.lock().unwrap() = Some(SystemTime::now());
                    let mut c = shared.collector.lock().unwrap();
                    if let Some(b) = c.buf.as_mut() {
                        b.extend_from_slice(&buf[..n]);
                    }
                    if c.matcher.feed(&buf[..n]) {
                        c.done = true;
                        shared.cv.notify_all();
                    }
                }
            }
        }
    });
}

fn wait_prompt(shared: &Shared, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut c = shared.collector.lock().unwrap();
    while !c.done && !c.died {
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        let (guard, _) = shared.cv.wait_timeout(c, deadline - now).unwrap();
        c = guard;
    }
    c.done
}

/// Execute one TCL operation to completion (the daemon side never
/// abandons a command; client timeouts are client-side only).
fn execute_tcl(shared: &Shared, tcl: &str) -> CommandResult {
    let started = SystemTime::now();

    // Latch the no-result marker first so stale output can never leak.
    let _ = write_json(&shared.paths.result(), &CommandResult::no_completed_command());
    {
        let mut m = shared.meta.lock().unwrap();
        m.state = SessionState::Busy;
        m.current_command = Some(tcl.to_string());
        m.current_command_started_at = Some(iso8601(started));
    }
    shared.write_meta();
    shared.log_line(&format!("command dispatch: {tcl}"));

    // Arm the collector, then send.
    {
        let mut c = shared.collector.lock().unwrap();
        c.buf = Some(Vec::new());
        c.done = false;
        c.matcher.reset();
    }
    {
        let mut w = shared.pty_writer.lock().unwrap();
        let _ = w.write_all(tcl.as_bytes());
        let _ = w.write_all(b"\n");
        let _ = w.flush();
    }

    // Wait for prompt or death — no daemon-side timeout.
    let (raw_output, died) = {
        let mut c = shared.collector.lock().unwrap();
        while !c.done && !c.died {
            c = shared.cv.wait(c).unwrap();
        }
        (c.buf.take().unwrap_or_default(), c.died && !c.done)
    };
    let finished = SystemTime::now();

    let text = String::from_utf8_lossy(&raw_output);
    let text = strip_prompt(&text);

    // Per-command suppression policy reload.
    let (policy, policy_err) = load_policy(&shared.workspace.join("elfws.yaml"));
    if let Some(e) = policy_err {
        shared.log_line(&e);
    }
    let mut filter = Filter::new(policy);
    for line in text.lines() {
        filter.feed_line(line);
    }
    let (mut output, mut errors) = filter.finish();

    if died {
        errors.push("Vivado process exited during the command".into());
    }

    // 1 MiB cap with explicit truncation marker.
    let mut truncated = false;
    if output.len() > RESULT_CAP {
        let mut cut = RESULT_CAP;
        while cut > 0 && !output.is_char_boundary(cut) {
            cut -= 1;
        }
        output.truncate(cut);
        output.push('\n');
        output.push_str(TRUNCATION_MARKER);
        truncated = true;
    }

    let result = CommandResult {
        completed: true,
        command: Some(tcl.to_string()),
        started_at: Some(iso8601(started)),
        finished_at: Some(iso8601(finished)),
        output: Some(output),
        had_errors: !errors.is_empty(),
        errors,
        truncated,
    };
    let _ = write_json(&shared.paths.result(), &result);
    {
        let mut m = shared.meta.lock().unwrap();
        m.state = if died { SessionState::Crashed } else { SessionState::Idle };
        m.current_command = None;
        m.current_command_started_at = None;
    }
    shared.write_meta();
    shared.log_line("command complete");
    result
}

fn handle_connection(shared: Arc<Shared>, mut stream: UnixStream) {
    let req: Request = match read_frame(&mut stream, MAX_REQUEST) {
        Ok(r) => r,
        Err(e) => {
            // Malformed/oversized requests are rejected, never fatal.
            let _ = write_frame(
                &mut stream,
                &Response::Error { final_: true, code: "protocol".into(), message: e.to_string() },
            );
            return;
        }
    };
    match req {
        Request::Ping => {
            let _ = write_frame(&mut stream, &Response::Ok { final_: true, message: "pong".into() });
        }
        Request::Exec { tcl } => handle_exec(&shared, &mut stream, tcl),
        Request::Stop => handle_stop(&shared, &mut stream),
        Request::RunTool { tool, argv } => handle_tool(&shared, &mut stream, tool, argv),
    }
}

/// Try to acquire the single-operation scheduler slot; on conflict,
/// reply `busy` immediately (never queue).
fn acquire_slot(shared: &Shared, stream: &mut UnixStream, description: &str) -> bool {
    let mut slot = shared.inflight.lock().unwrap();
    if let Some(cur) = slot.as_ref() {
        let last_read = shared.last_pty_read.lock().unwrap().map(iso8601);
        let _ = write_frame(
            stream,
            &Response::Busy {
                final_: true,
                current_command: cur.command.clone(),
                dispatched_at: cur.dispatched_at.clone(),
                last_pty_read_at: last_read,
            },
        );
        return false;
    }
    *slot = Some(InFlight { command: description.to_string(), dispatched_at: iso8601_now() });
    true
}

fn release_slot(shared: &Shared) {
    *shared.inflight.lock().unwrap() = None;
}

fn handle_exec(shared: &Shared, stream: &mut UnixStream, tcl: String) {
    if shared.collector.lock().unwrap().died {
        let _ = write_frame(
            stream,
            &Response::Error {
                final_: true,
                code: "crashed".into(),
                message: "Vivado process is not running".into(),
            },
        );
        return;
    }
    if !acquire_slot(shared, stream, &tcl) {
        return;
    }
    let result = execute_tcl(shared, &tcl);
    release_slot(shared);
    // Failure to deliver must not crash the daemon: ignore write errors.
    let _ = write_frame(
        stream,
        &Response::Result {
            final_: true,
            result: serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
        },
    );
}

fn handle_stop(shared: &Shared, stream: &mut UnixStream) {
    {
        let slot = shared.inflight.lock().unwrap();
        if let Some(cur) = slot.as_ref() {
            let _ = write_frame(
                stream,
                &Response::Busy {
                    final_: true,
                    current_command: cur.command.clone(),
                    dispatched_at: cur.dispatched_at.clone(),
                    last_pty_read_at: shared.last_pty_read.lock().unwrap().map(iso8601),
                },
            );
            return;
        }
    }
    let _ = write_frame(
        &mut *stream,
        &Response::Ok { final_: true, message: "shutting down".into() },
    );
    shared.shutdown.store(true, Ordering::SeqCst);
}

/// Managed tool operation (task 2.5). Shares the one scheduler slot with
/// TCL; argv/stdio/exit status preserved verbatim.
fn handle_tool(shared: &Shared, stream: &mut UnixStream, tool: String, argv: Vec<String>) {
    if !TOOLS.contains(&tool.as_str()) {
        let _ = write_frame(
            stream,
            &Response::Error { final_: true, code: "usage".into(), message: format!("unknown tool {tool}") },
        );
        return;
    }
    if tool == "xsct" && argv.is_empty() {
        let _ = write_frame(
            stream,
            &Response::Error { final_: true, code: "usage".into(), message: "xsct requires a TCL file".into() },
        );
        return;
    }
    if !acquire_slot(shared, stream, &format!("run {tool}")) {
        return;
    }

    // Working directory: xsct runs from its TCL file's directory.
    let mut cwd = shared.workspace.clone();
    let mut tool_argv = argv.clone();
    if tool == "xsct" {
        let f = Path::new(&argv[0]);
        let abs = if f.is_absolute() { f.to_path_buf() } else { shared.workspace.join(f) };
        if !abs.is_file() {
            release_slot(shared);
            let _ = write_frame(
                stream,
                &Response::Error {
                    final_: true,
                    code: "usage".into(),
                    message: format!("xsct TCL file not found: {}", abs.display()),
                },
            );
            return;
        }
        cwd = abs.parent().unwrap_or(&shared.workspace).to_path_buf();
        tool_argv[0] = abs.display().to_string();
    }

    let started = iso8601_now();
    let op = ToolOperation {
        tool: tool.clone(),
        argv: argv.clone(),
        cwd: cwd.display().to_string(),
        started_at: started.clone(),
        finished_at: None,
        state: "running".into(),
        exit_status: None,
    };
    {
        let mut m = shared.meta.lock().unwrap();
        m.state = SessionState::Busy;
        m.current_tool_operation = Some(op.clone());
    }
    shared.write_meta();
    shared.log_line(&format!("tool operation start: {tool} {argv:?}"));

    // xsct/xsdb need the Vitis environment; bootgen/dtc run directly.
    let needs_vitis = tool == "xsct" || tool == "xsdb";
    let mut cmd = if needs_vitis {
        let mut c = std::process::Command::new("bash");
        // Source the vendor settings, then exec the tool with argv
        // preserved as positional parameters — no eval of arg content.
        c.arg("-c")
            .arg("source \"${VITIS_PATH:?VITIS_PATH not set}/settings64.sh\" >/dev/null 2>&1; exec \"$0\" \"$@\"")
            .arg(&tool)
            .args(&tool_argv);
        c
    } else {
        let mut c = std::process::Command::new(&tool);
        c.args(&tool_argv);
        c
    };
    if Path::new(UDEV_STUB).exists() {
        cmd.env("LD_PRELOAD", UDEV_STUB);
    }
    cmd.current_dir(&cwd)
        .env("HOME", shared.workspace.display().to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let exit_status = match cmd.spawn() {
        Err(e) => {
            let _ = write_frame(
                stream,
                &Response::Error { final_: true, code: "internal".into(), message: format!("spawn {tool}: {e}") },
            );
            finish_tool(shared, op, None);
            release_slot(shared);
            return;
        }
        Ok(mut child) => {
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            let ws = Arc::new(Mutex::new(stream.try_clone().ok()));
            let t1 = stream_pipe(shared, ws.clone(), out, 1);
            let t2 = stream_pipe(shared, ws.clone(), err, 2);
            let status = child.wait();
            let _ = t1.join();
            let _ = t2.join();
            status.ok().and_then(|s| s.code()).unwrap_or(-1)
        }
    };

    shared.log_line(&format!("tool operation end: {tool} exit={exit_status}"));
    finish_tool(shared, op, Some(exit_status));
    release_slot(shared);
    let _ = write_frame(stream, &Response::Exit { final_: true, exit_status });
}

fn finish_tool(shared: &Shared, mut op: ToolOperation, exit_status: Option<i32>) {
    op.finished_at = Some(iso8601_now());
    op.exit_status = exit_status;
    op.state = match exit_status {
        Some(0) => "completed".into(),
        Some(_) => "failed".into(),
        None => "failed".into(),
    };
    let mut m = shared.meta.lock().unwrap();
    m.current_tool_operation = None;
    m.last_tool_operation = Some(op);
    m.state = SessionState::Idle;
    drop(m);
    shared.write_meta();
}

/// Stream a tool pipe to the client as `Stream` frames while also
/// appending to the raw audit log.
fn stream_pipe(
    shared: &Shared,
    ws: Arc<Mutex<Option<UnixStream>>>,
    mut pipe: impl std::io::Read + Send + 'static,
    fd: u8,
) -> std::thread::JoinHandle<()> {
    let raw = shared.raw_log.lock().unwrap().try_clone().ok();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut raw = raw;
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if let Some(f) = raw.as_mut() {
                        let _ = f.write_all(&buf[..n]);
                    }
                    let mut guard = ws.lock().unwrap();
                    if let Some(s) = guard.as_mut() {
                        let frame = Response::Stream {
                            final_: false,
                            fd,
                            data: String::from_utf8_lossy(&buf[..n]).into_owned(),
                        };
                        if write_frame(s, &frame).is_err() {
                            *guard = None; // client gone; keep draining for the log
                        }
                    }
                }
            }
        }
    })
}

/// Health sampler: every ~10 s, sample the Vivado tree over ~1 s.
fn spawn_health_sampler(shared: Arc<Shared>) {
    std::thread::spawn(move || loop {
        let s = crate::procfs::sample_tree(shared.vivado_pid, 1000);
        let last = *shared.last_pty_read.lock().unwrap();
        let age = last
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(-1.0);
        let sample = HealthSample {
            sampled_at: iso8601_now(),
            descendants: s.live_non_zombie,
            cpu_percent: s.cpu_percent,
            rss_kib: s.rss_kib,
            last_pty_read_at: last.map(iso8601),
            last_pty_read_age_seconds: (age * 10.0).round() / 10.0,
        };
        let _ = write_json(&shared.paths.health(), &sample);
        let _ = unix_now();
        std::thread::sleep(Duration::from_secs(9));
    });
}
