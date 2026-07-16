//! dv — daVit CLI: manage a containerized Vivado/Vitis session.
//!
//! Grammar: `dv <verb> [object] [parameters]`. Arguments are parsed as
//! an argv array — never reconstructed or reparsed through a shell.
//! There is no wildcard fallback treating unknown verbs as TCL.

mod artifacts;
mod client;
mod daemon;
mod filter;
mod procfs;
mod protocol;
mod pty;
mod util;

use client::{usage_err, EXIT_OK, EXIT_USAGE};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
dv — daVit: containerized Vivado/Vitis session CLI

USAGE:
    dv <verb> [object] [parameters]

VERBS:
    start [headless|gui]   Create and start the session container
                           (host launcher only; sidecars use the
                           orchestrator for lifecycle)
    stop [--force]         Gracefully stop the session; refuses while a
                           command is in flight (--force needs the host
                           launcher and may lose the in-flight result)
    exec [--timeout S] [--file F] [--] [TCL ...]
                           Run one TCL operation in the persistent session
    show <status|result|metadata|health> [--json]
                           Read current session state from artifacts
    logs [--tail N] [--follow]
                           Read the raw session log directly
    diagnose <last|metadata|health|inspect|logs|ps|wchan|fionread|fdtable>
                           Read-only probes from artifacts and procfs;
                           never touches the daemon socket or PTY
    run <xsct|xsdb|bootgen|dtc> [args ...]
                           Run a managed Vitis/companion tool operation

EXIT CODES:
    0  success   1  Vivado error / busy / crash   2  usage or no result
    3  client wait timed out; the command continues in the session

WEDGE HEURISTIC:
    Large PTY-idle time with high descendant CPU (see `show health`)
    means a quiet active phase; large PTY-idle with zero descendants and
    zero CPU suggests a wedge. dv reports this but never kills a session
    automatically.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dispatch(args);
    std::process::exit(code);
}

fn dispatch(args: Vec<String>) -> i32 {
    let Some(verb) = args.first().map(String::as_str) else {
        eprint!("{HELP}");
        return EXIT_USAGE;
    };
    let rest = &args[1..];

    if verb == "--help" || verb == "-h" || verb == "help" {
        print!("{HELP}");
        return EXIT_OK;
    }
    if verb == "--version" || verb == "-V" {
        println!("dv {VERSION} (protocol {})", artifacts::PROTOCOL_VERSION);
        return EXIT_OK;
    }
    if wants_help(rest) {
        print!("{HELP}");
        return EXIT_OK;
    }

    match verb {
        // Hidden: the container entrypoint runs the daemon.
        "_daemon" => {
            let mut project = None;
            let mut workspace = PathBuf::from("/workspace");
            let mut it = rest.iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--project" => project = Some(required(&mut it, "--project")),
                    "--workspace" => workspace = PathBuf::from(required(&mut it, "--workspace")),
                    other => usage_err(&format!("_daemon: unknown option {other}")),
                }
            }
            daemon::run(&workspace, project)
        }
        // Hidden: image HEALTHCHECK.
        "_healthcheck" => client::healthcheck(),

        "start" => {
            // The published binary cannot create containers. Lifecycle is
            // owned by the host launcher (scripts/dv) or the orchestrator.
            eprintln!(
                "dv: `start` needs a container runtime and is handled by the host \
                 launcher (scripts/dv). In a sidecar setup, lifecycle is owned by \
                 the orchestrator (e.g. docker compose)."
            );
            EXIT_USAGE
        }

        "stop" => {
            let mut force = false;
            for a in rest {
                match a.as_str() {
                    "--force" => force = true,
                    "--timeout" => {} // accepted for launcher compatibility
                    v if v.parse::<u64>().is_ok() => {}
                    other => usage_err(&format!("stop: unknown option {other}")),
                }
            }
            client::stop(force)
        }

        "exec" => {
            let mut timeout = None;
            let mut file = None;
            let mut inline = Vec::new();
            let mut after_dashdash = false;
            let mut it = rest.iter();
            while let Some(a) = it.next() {
                if after_dashdash {
                    inline.push(a.clone());
                    continue;
                }
                match a.as_str() {
                    "--" => after_dashdash = true,
                    "--timeout" => {
                        timeout = Some(
                            required(&mut it, "--timeout")
                                .parse::<u64>()
                                .unwrap_or_else(|_| usage_err("--timeout takes seconds")),
                        )
                    }
                    "--file" => file = Some(required(&mut it, "--file")),
                    _ => inline.push(a.clone()),
                }
            }
            client::exec(client::ExecArgs { timeout, file, inline })
        }

        "show" => {
            let Some(object) = rest.first() else {
                usage_err("show takes one of: status, result, metadata, health")
            };
            let json = rest.iter().any(|a| a == "--json");
            client::show(object, json)
        }

        "logs" => {
            let (mut tail, mut follow) = (50usize, false);
            let mut it = rest.iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--tail" => {
                        tail = required(&mut it, "--tail")
                            .parse()
                            .unwrap_or_else(|_| usage_err("--tail takes a number"))
                    }
                    "--follow" | "-f" => follow = true,
                    other => usage_err(&format!("logs: unknown option {other}")),
                }
            }
            client::logs(tail, follow)
        }

        "diagnose" => {
            let Some(probe) = rest.first() else {
                usage_err(
                    "diagnose takes one of: last, metadata, health, inspect, logs, ps, wchan, fionread, fdtable",
                )
            };
            let json = rest.iter().any(|a| a == "--json");
            let mut tail = 50usize;
            let mut it = rest.iter();
            while let Some(a) = it.next() {
                if a == "--tail" {
                    tail = required(&mut it, "--tail")
                        .parse()
                        .unwrap_or_else(|_| usage_err("--tail takes a number"));
                }
            }
            client::diagnose(probe, tail, json)
        }

        "run" => {
            let Some(tool) = rest.first() else {
                usage_err("run takes one of: xsct, xsdb, bootgen, dtc")
            };
            client::run_tool(tool, rest[1..].to_vec())
        }

        other => {
            eprintln!(
                "dv: unknown verb `{other}` (TCL execution always goes through `dv exec`)\n"
            );
            eprint!("{HELP}");
            EXIT_USAGE
        }
    }
}

fn wants_help(rest: &[String]) -> bool {
    rest.iter().any(|a| a == "--help" || a == "-h")
}

fn required(it: &mut std::slice::Iter<String>, opt: &str) -> String {
    it.next().cloned().unwrap_or_else(|| usage_err(&format!("{opt} requires a value")))
}
