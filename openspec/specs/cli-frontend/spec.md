# cli-frontend Specification

## Purpose
Define the daVit `dv` CLI grammar, host launcher behavior, sidecar
operation, and verb semantics for the persistent session workflow.

## Requirements

### Requirement: Verb grammar without TCL fallthrough

The `dv` CLI SHALL implement the grammar
`dv <verb> [object] [parameters]` with reserved verbs `start`
(`headless` default | `gui`), `stop`, `exec`, `show`
(`status|result|metadata|health`), `logs`, `diagnose`, and `run`
(`xsct|xsdb|bootgen|dtc`). Arguments SHALL be parsed as argv without
`eval` or shell reparsing. There SHALL be no wildcard fallback treating
unknown verbs as TCL. Every verb SHALL support `--help`; the top level
SHALL support `--help` and `--version` (reporting the tool/protocol
version). Usage errors SHALL print to stderr and exit `2`.

#### Scenario: Misspelled verb is not TCL
- **WHEN** a user runs `dv stats`
- **THEN** the CLI prints a usage error to stderr and exits `2` without
  contacting the session

#### Scenario: Metacharacters survive argv
- **WHEN** `dv exec 'puts "a b;c"'` is invoked
- **THEN** the TCL string received by Vivado is exactly `puts "a b;c"`

### Requirement: Launcher owns runtime verbs only

The repo SHALL ship a thin host launcher `scripts/dv` whose sole
runtime responsibilities are: `start` (validate inputs, create the
container via the OCI runtime with `-u "$(id -u):$(id -g)"`, workspace
bind-mounted at `/workspace`, `--init`, then wait for readiness) and
the `--force` escalation of `stop` (runtime SIGTERM/kill when the
daemon is unreachable). Every other invocation SHALL be delegated
verbatim to `<workspace>/.dv/bin/dv`. When no session root exists, the
launcher SHALL print a hint to run `dv start` and exit `2`.

Container names SHALL be per-mode: headless sessions use
`davit-<wshash>` and GUI sessions use `davit-<wshash>-gui`, where
`<wshash>` is the deterministic workspace hash. GUI containers SHALL
carry the label `davit.session=gui` in addition to the workspace
ownership label. The `start` guard SHALL inspect only the container
name belonging to the requested mode. `start` SHALL NOT replace or
adopt an unrelated container; repeated `start headless` of the same
healthy session SHALL be idempotent, and `start gui` while a GUI
container for this workspace is running SHALL be refused with a
non-zero exit. Stale stopped containers owned by this workspace SHALL
be removed before creating a replacement, for either mode.

`stop --force` SHALL stop and remove every workspace-owned container
among `davit-<wshash>` and `davit-<wshash>-gui`, and SHALL error only
when neither exists.

#### Scenario: Delegation to published binary
- **WHEN** the host user runs `scripts/dv exec 'get_projects'` in a
  workspace with a live session
- **THEN** the launcher execs `.dv/bin/dv exec get_projects` and the
  exit status is the published binary's

#### Scenario: Duplicate start is idempotent
- **WHEN** `dv start headless` runs while the managed session container
  is already healthy
- **THEN** the command reports current status and exits `0` without
  creating a second container

#### Scenario: Duplicate GUI start is refused
- **WHEN** `dv start gui` runs while `davit-<wshash>-gui` for this
  workspace is already running
- **THEN** the launcher prints a "gui already open" error and exits
  non-zero without creating a container

#### Scenario: Force stop sweeps both containers
- **WHEN** `dv stop --force` runs while both a headless session and a
  GUI container exist for this workspace
- **THEN** both containers are stopped and removed, and the command
  reports each removal

### Requirement: GUI and headless sessions coexist per workspace

`dv start gui` SHALL succeed while a headless session container is
running in the same workspace, and `dv start headless` SHALL succeed
while a GUI container is running. The GUI container SHALL remain
session-free: no daemon, no control socket, and no writes under
`.dv/`; all delegated verbs (`exec`, `show`, `logs`, `diagnose`,
`run`) SHALL continue to address the headless session exclusively.
Concurrent access to the same project from both containers is the
user's responsibility; the launcher SHALL NOT add project-level
locking.

#### Scenario: GUI opens alongside a running batch session
- **WHEN** a headless session is running and the user runs
  `dv start gui`
- **THEN** a GUI container named `davit-<wshash>-gui` starts, and
  `dv exec` against the headless session continues to work while the
  GUI is open

#### Scenario: GUI leaves no session artifacts
- **WHEN** a GUI container runs and exits
- **THEN** the contents of `<workspace>/.dv/` are unchanged by the GUI
  container

### Requirement: Sidecar operation without a container runtime

All session verbs SHALL function using only the shared workspace — the
socket and artifact files under `.dv/` — covering `exec`, graceful
`stop`, `show`, `logs`, `diagnose`, and `run`. A sidecar invoking `start` (or
`stop --force`) SHALL receive a clear error stating lifecycle is owned
by the orchestrator. Graceful `stop` SHALL be a socket request that
refuses (non-zero, reporting the command and elapsed time) while a
command is in flight.

#### Scenario: Graceful stop from a sibling
- **WHEN** a sibling container runs `dv stop` against an idle session
- **THEN** the daemon shuts down cleanly and the session container
  exits, with no Docker access required by the sibling

#### Scenario: Stop refuses mid-command
- **WHEN** `dv stop` is issued while `launch_runs` is executing
- **THEN** the CLI exits non-zero, reporting the in-flight command and
  elapsed time, and the command continues

### Requirement: exec semantics

`dv exec [--timeout SECONDS] [--file TCL_FILE] [--] [TCL ...]` SHALL
require exactly one of inline TCL or `--file`. Inline arguments SHALL
be joined with single spaces after argv parsing; `--file` SHALL submit
the complete file content as one operation. On client timeout the CLI
SHALL exit `3` and direct the caller to `dv show result`. Exit codes:
`0` success, `1` Vivado error/critical warning/busy/crash, `2` usage or
no result, `3` client timeout. Vivado process death SHALL be
distinguished on stderr from a TCL-reported error.

#### Scenario: Timeout directs to result
- **WHEN** `dv exec --timeout 5 'after 60000'` times out client-side
- **THEN** the CLI exits `3` with a message referencing
  `dv show result`, and the result is retrievable after completion

### Requirement: Machine-readable output

Verbs supporting `--json` SHALL write exactly one valid JSON value to
stdout with informational messages on stderr. Timestamps SHALL be ISO
8601 with offset or `Z`; durations SHALL include numeric seconds. JSON
keys SHALL remain stable within a protocol major version;
`metadata.json` SHALL carry the protocol version and the CLI SHALL warn
on incompatibility. Missing optional data in composite output
(`diagnose inspect --json`) SHALL be `null`, never fabricated.

#### Scenario: Single JSON value
- **WHEN** `dv show status --json` runs
- **THEN** stdout parses as one JSON document and any warnings appear
  only on stderr

### Requirement: Logs read files, never Vivado

`dv logs [--tail N] [--follow]` SHALL read the raw session log file
directly. Without `--follow` it SHALL print a finite snapshot (default
tail 50) and exit. It SHALL NOT dispatch TCL or open the control
socket.

#### Scenario: Logs during a busy session
- **WHEN** `dv logs --tail 100` runs while a command is in flight
- **THEN** the tail is printed from the log file and the in-flight
  command is unaffected

### Requirement: GUI start profiles and JTAG option

`dv start gui` SHALL support host profiles `x11`, `wayland`, and `wsl`
with auto-detection (`/proc/version` for WSL, `WAYLAND_DISPLAY` for
Wayland, else X11) and `--gui-profile` override, binding display
sockets/authority read-only. `--jtag-host[=HOST:PORT]` SHALL resolve
inline value, then configuration/environment, then
`host.docker.internal:3121`; the two-token form `--jtag-host URL` SHALL
be rejected. GUI mode SHALL imply `--jtag-host`. A failed reachability
probe SHALL warn (mentioning `hw_server -d`) but not block startup.

#### Scenario: JTAG default resolution
- **WHEN** `dv start headless --jtag-host` runs with no configured URL
- **THEN** the container receives
  `VIVADO_HW_SERVER_URL=host.docker.internal:3121` and a probe warning
  appears only if `localhost:3121` is unreachable
