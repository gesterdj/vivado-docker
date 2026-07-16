# vivado-session Specification

## Purpose
Define the persistent session container: modes, init contract, session
root, readiness gating, serialized TCL execution, output filtering,
health sampling, managed tool operations, udev stub, and JTAG brokering.

## Requirements

### Requirement: One container, one session, non-root

A session container SHALL run exactly one Vivado/Vitis workflow in one
of two mutually exclusive modes fixed at creation: `headless` (one
persistent Vivado TCL session named `main`) or `gui`
(`vivado -mode gui`). The container SHALL run as the non-root
`-u UID:GID` identity supplied at creation. The entrypoint SHALL fail
fast when running as root, when UID/GID are absent, or when
`/workspace` is not writable. It SHALL NOT chown any path outside the
session root and SHALL NOT start an SSH service.

#### Scenario: Root rejected
- **WHEN** the container starts without `-u UID:GID` (effective UID 0)
- **THEN** the entrypoint exits non-zero with a clear message before
  any application process starts

#### Scenario: Mode is fixed
- **WHEN** a container was created in `gui` mode
- **THEN** no daemon, control socket, or headless session artifacts are
  created, and session commands against it fail clearly

### Requirement: Foreground process and init contract

The container SHALL use a minimal init as PID 1 that reaps orphans and
forwards SIGTERM. In headless mode the daemon is the foreground
application; in GUI mode Vivado is. Container lifetime SHALL equal
foreground-application lifetime; on SIGTERM an idle session SHALL exit
within the runtime grace period.

#### Scenario: Graceful stop of idle session
- **WHEN** `docker stop` is issued against an idle headless container
- **THEN** the daemon shuts Vivado down and the container exits without
  SIGKILL, leaving no zombie processes

### Requirement: Session root in the shared workspace

Headless startup SHALL create the session root `<workspace>/.dv/`
containing: `control.sock` (Unix domain socket, owner-only
permissions), `metadata.json`, `result.json`, `health.json`, a
timestamped raw log, and `bin/dv` — a copy of the container's own
`davit` binary published for callers. No TCP port SHALL be bound.
Missing or unwritable session storage SHALL fail startup.

#### Scenario: Sidecar finds everything via the mount
- **WHEN** a sibling container mounts the same workspace and runs
  `.dv/bin/dv show status`
- **THEN** the command succeeds using only the socket and files under
  `.dv/`, with no software preinstalled in the sibling image

#### Scenario: Self-published binary matches the daemon
- **WHEN** the daemon starts
- **THEN** `.dv/bin/dv` is byte-identical to the binary running the
  daemon

### Requirement: Readiness gating

The headless session SHALL be ready only after Vivado emits its prompt
and any requested project (from `--project`/`DV_PROJECT`) has opened
successfully. Metadata SHALL expose state
`starting|idle|busy|crashed|stopped|unreachable`. Startup failure SHALL
terminate the container with a non-zero exit. The image SHALL define a
`HEALTHCHECK` that reports healthy only when the session is ready.

#### Scenario: Orchestrator gates on health
- **WHEN** a compose sibling declares
  `depends_on: {vivado: {condition: service_healthy}}`
- **THEN** the sibling starts only after the Vivado prompt is up and
  the project (if any) is open

#### Scenario: Bad project fails start
- **WHEN** the configured project path does not exist, lies outside the
  workspace, or lacks the `.xpr` suffix
- **THEN** the container exits non-zero and the health check never
  reports healthy

### Requirement: Serialized TCL execution with latched results

The daemon SHALL own Vivado through a PTY and execute at most one
operation at a time. A command arriving while one is in flight SHALL be
rejected immediately with status `busy`, the current command, dispatch
time, and last PTY-read time; it SHALL NOT queue. Dispatch SHALL first
replace `result.json` with a durable `no completed command` marker;
completion SHALL write the result via temp-file + atomic rename with
`command`, `started_at`, `finished_at`, `output`, `errors`,
`had_errors`, `truncated` fields. Filtered output SHALL be capped at
1 MiB with the explicit marker
`[output truncated at 1048576 bytes]`. Client disconnects and
client-side timeouts SHALL NOT interrupt the running command; the
eventual result SHALL still be latched.

#### Scenario: Busy rejection
- **WHEN** a TCL command is in flight and a second `exec` arrives
- **THEN** the second caller receives a `busy` response naming the
  in-flight command, and the interpreter receives nothing from it

#### Scenario: Client timeout does not kill the command
- **WHEN** a client disconnects after its wait timeout during a long
  `launch_runs`
- **THEN** the daemon runs the command to completion and `show result`
  later returns the latched result

### Requirement: Output filtering with raw audit log

All raw Vivado output SHALL be appended unfiltered to the session raw
log. User-facing output SHALL pass a stateful line filter: `INFO:`
suppressed with continuations; `ERROR:` and `CRITICAL WARNING:`
retained and surfaced as errors; `WARNING:` suppressed by default, with
a workspace `elfws.yaml` suppression file selectively retaining IDs not
listed in it. The file SHALL be reloaded per command; a parse failure
SHALL log to the raw log and fall back to blanket warning suppression
without crashing the session.

#### Scenario: Warnings suppressed by default
- **WHEN** no `elfws.yaml` exists and a command emits `WARNING:` lines
- **THEN** the filtered result omits them while the raw log contains
  them verbatim

#### Scenario: Suppression file reload
- **WHEN** `elfws.yaml` is edited between two commands
- **THEN** the second command's filtering reflects the edit without a
  session restart

### Requirement: Health sampling and read-only diagnostics

The daemon SHALL sample the Vivado process tree (live descendant count,
aggregate CPU %, aggregate RSS, last PTY-read time) roughly every ten
seconds, written atomically to `health.json`. Diagnostic probes (`last`,
`metadata`, `health`, `inspect`, `logs`, `ps`, `wchan`, `fionread`,
`fdtable`) SHALL be served exclusively from session artifacts and
procfs — never by opening the control socket, reading the PTY, or
dispatching TCL. A live-looking metadata file with an unreachable
socket SHALL be reported as `unreachable`; read-only commands SHALL NOT
rewrite state.

#### Scenario: Diagnostics cannot recurse into Vivado
- **WHEN** any `diagnose` probe runs against a busy session
- **THEN** no bytes are read from the daemon PTY and no socket request
  is made, and the in-flight command is unaffected

### Requirement: Managed tool operations share the session

`run xsct|xsdb|bootgen|dtc` operations SHALL execute as daemon-spawned
children inside the session container, registered in metadata with tool
name, argv, working directory, timestamps, state, and exit status, and
recorded in the raw log under timestamped headers. One scheduler SHALL
serialize TCL and tool operations. Tool argv, stdio, and exit status
SHALL be preserved verbatim; `xsct`/`xsdb` SHALL run with the Vitis
environment initialized, `xsct` from the TCL file's directory.
Operations SHALL NOT outlive the container.

#### Scenario: Tool exit code preserved
- **WHEN** `run bootgen` exits with status 3
- **THEN** the caller observes exit status 3 and unmodified
  stdout/stderr streams

#### Scenario: Tool rejected while TCL in flight
- **WHEN** a TCL command is running and `run xsct` arrives
- **THEN** the operation is rejected as busy rather than run
  concurrently

### Requirement: udev stub scoped to Vivado processes

The libudev stub at `/opt/udev_stub.so` SHALL be preloaded for Vivado
and Vitis tool process trees in both modes, as a universal
Vivado-in-Docker mitigation. Ad-hoc shells and diagnostics SHALL
resolve the system libudev.

#### Scenario: open_project survives licensing stack
- **WHEN** `exec open_project` runs in a headless session
- **THEN** no crash occurs in the WebTalk/licensing stack

### Requirement: Host-brokered JTAG environment

When JTAG brokering is enabled at start, the container SHALL resolve
`host.docker.internal` to the host gateway and SHALL expose the
resolved URL as `VIVADO_HW_SERVER_URL`. The session SHALL NOT
auto-connect to hardware, mount `/dev/bus/usb` or `/run/udev`, or add
device-cgroup rules.

#### Scenario: Manual connect uses the env URL
- **WHEN** a caller executes
  `exec 'connect_hw_server -url $::env(VIVADO_HW_SERVER_URL)'`
- **THEN** the connection targets the host `hw_server` without any USB
  passthrough
