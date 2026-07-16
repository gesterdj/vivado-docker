# cli-frontend — Delta Specification

## ADDED Requirements

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
launcher SHALL print a hint to run `dv start` and exit `2`. `start`
SHALL NOT replace or adopt an unrelated container; repeated `start` of
the same healthy session SHALL be idempotent.

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
