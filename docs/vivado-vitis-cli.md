# Containerized Vivado/Vitis CLI — Behavioral Specification

## 1. Status and intent

This document specifies a fresh command-line interface for creating, managing,
and interacting with a containerized AMD/Xilinx Vivado 2024.2 and Vitis 2024.2
environment. It consolidates the behavior described by the current container
and `vivado-cli` documentation and OpenSpec projects, while deliberately leaving
the implementation language, repository layout, module boundaries, and installed
file locations open.

The normative words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY**
are to be interpreted as requirement levels. Examples use the neutral
`<script>` placeholder; the implementation name is intentionally unspecified.

The intended grammar is consistently:

```text
<script> <verb> [object] [parameters]
```

Examples:

```sh
<script> start headless --project ./design.xpr
<script> exec 'get_projects'
<script> show status
<script> show result
<script> diagnose inspect --json
<script> run xsct ./scripts/mkfsbl.tcl release
<script> stop
```

This is a desired-state specification, not a compatibility promise for the
existing scripts. In particular, arbitrary-command fallthrough is replaced by
the explicit `exec` verb, and tool names are objects of the explicit `run` verb.

## 2. Goals

The system MUST:

1. Treat one container as one integrated Vivado/Vitis workflow environment.
2. Make the host-side CLI the stable public interface; callers MUST NOT need to
   know internal process names, Unix-socket paths, or source-tree layout.
3. Support a persistent, serialized Vivado TCL interpreter in headless mode.
4. Support an interactive Vivado GUI as a mutually exclusive container mode.
5. Manage Vitis-side batch tools as operations in the same container workflow,
   lifecycle, status model, and audit trail as Vivado operations.
6. Preserve long-running Vivado work across client disconnects and client-side
   timeouts.
7. Provide safe, read-only observability that cannot recursively invoke Vivado.
8. Preserve host ownership of files written into the mounted workspace.
9. Keep hardware access on a host `hw_server` broker rather than passing USB
   devices into the container.
10. Expose predictable stdout, stderr, exit codes, and JSON suitable for humans,
    scripts, CI, and agent-driven workflows.

## 3. Non-goals

The first implementation MUST NOT require or prescribe:

- a particular repository or package directory structure;
- multiple persistent Vivado sessions in one container;
- switching a running container between headless and GUI modes;
- a TCP listener for the Vivado TCL protocol;
- direct USB/JTAG device passthrough, host udev-rule installation, VNC, Xpra,
  PulseAudio, GPU passthrough, or native Wayland rendering;
- a general unprivileged shell verb or script-file execution verb;
- automatic connection to a hardware server;
- emulation of Vivado, Vitis, Docker, the host display server, or `hw_server`.

Supporting Podman or another OCI runtime MAY be added, but Docker-compatible
semantics are the baseline.

## 4. Public command grammar

### 4.1 General rules

The CLI MUST parse arguments as an argv array and MUST NOT reconstruct or parse
them with shell `eval`. The CLI MUST preserve each argument passed after the
verb/object boundary. Parameters may precede or follow the object only where the
command synopsis explicitly permits it.

Every verb MUST support `--help`. The top-level command MUST support `--help` and
`--version`. Unknown verbs, objects, options, and missing required values MUST
print a concise usage error to stderr and exit `2`.

The reserved top-level verbs are:

| Verb | Object | Purpose |
|---|---|---|
| `start` | `headless` or `gui` | Create and start the container |
| `stop` | none | Gracefully terminate the managed container |
| `show` | `status`, `result`, `metadata`, or `health` | Read current state |
| `exec` | TCL command | Execute TCL in the persistent Vivado session |
| `logs` | none | Read or follow the raw Vivado log |
| `diagnose` | diagnostic probe | Perform read-only session inspection |
| `run` | `xsct`, `xsdb`, `bootgen`, or `dtc` | Run a managed workflow operation |

There MUST be no wildcard fallback that treats an unrecognized verb as TCL.
This prevents spelling mistakes such as `<script> stats` from becoming valid
Vivado commands. TCL execution MUST always be introduced by `<script> exec`.

### 4.2 Common options

The implementation SHOULD accept these common host-side options before the verb:

```text
--container NAME       managed container name or identifier
--runtime PROGRAM      OCI runtime executable; default "docker"
--json                 machine-readable output where supported
--quiet                suppress non-result informational output
```

Selection precedence MUST be command option, configuration, environment, then
documented default. Secrets and private-key content MUST NOT be accepted through
ordinary command-line options because process arguments may be visible to other
host users.

## 5. Container lifecycle

### 5.1 Start

```text
<script> start [headless|gui] [options]

Options:
  --project HOST_PATH
  --workspace HOST_DIR
  --image IMAGE
  --name NAME
  --jtag-host[=HOST:PORT]
  --ssh-key PUBLIC_KEY_PATH
  --gui-profile x11|wayland|wsl
  --detach
```

`headless` MUST be the default object when no mode is given. `start` MUST fail
without replacing or adopting an unrelated existing container with the selected
name. A repeated start of the same healthy managed container SHOULD be
idempotent and return its status; it MUST NOT create a second container.

The workspace defaults to the current directory and MUST be bind-mounted at one
stable path in the container. The exact in-container pathname is intentionally
unspecified; all paths supplied by the host CLI MUST be translated consistently.
This resolves the current documentation conflict between `/project` and
`/workspace` without baking either layout into the new architecture.

If `--project` is relative, it MUST be resolved against the host workspace. The
file MUST exist, be inside the mounted workspace, and have an `.xpr` suffix.
Failure MUST occur before container creation. If no project is given, headless
Vivado starts without an open project and GUI Vivado opens its welcome screen.

The launcher MUST pass the host operator's numeric UID and GID. Container startup
MUST fail clearly if either value is absent. Before application processes start,
the runtime entrypoint MUST align the unprivileged tool user to those IDs and
ensure that files created in the workspace are host-editable. It MUST NOT
recursively change ownership of application or system directories. If IDs
already match, the rewrite SHOULD be skipped.

### 5.2 Runtime process contract

The container MUST use a minimal init as PID 1 that reaps orphaned descendants
and forwards termination signals. The foreground application MUST remain a
direct descendant of that init, and the container lifetime MUST remain coupled
to the foreground application lifetime.

On `docker stop` or equivalent, SIGTERM MUST reach the foreground application.
An idle container SHOULD exit within the runtime's normal grace period without
SIGKILL. The init MUST return the foreground application's status.

### 5.3 Headless mode

Headless mode MUST start exactly one persistent Vivado TCL session named
conceptually `main`. The session process stays in the foreground from the
container runtime's perspective; an internal supervisor MAY own the Vivado PTY
and control socket.

When a public key is supplied, headless mode MUST start an SSH service with
key-only authentication for the unprivileged tool user. When no key is supplied,
SSH MUST remain off. The same command semantics MAY also be transported through
local runtime execution. Transport choice MUST NOT alter command output or exit
status.

The headless session is ready only after Vivado emits its prompt and any requested
project has opened successfully. `start` MUST not report success before readiness.
Startup failure MUST terminate the container and produce a non-zero exit.

### 5.4 GUI mode

GUI mode MUST be selected at container creation and MUST remain fixed for the
container lifetime. It MUST run `vivado -mode gui` as the foreground application.
It MUST NOT start SSH, the persistent TCL daemon, a session control socket, or a
diagnostic session surface.

The launcher MUST support host X11, XWayland, and WSLg profiles:

- `wsl` when `/proc/version` identifies Microsoft/WSL;
- `wayland` when `WAYLAND_DISPLAY` is present;
- `x11` otherwise.

An explicit `--gui-profile` MUST override detection. The launcher MUST pass the
needed display variables and bind the appropriate Unix sockets and authority
file read-only. It MUST NOT use SSH X11 forwarding. Closing the Vivado window
MUST end the container.

`--detach` MAY be used for headless mode. GUI mode SHOULD remain attached by
default so terminal interrupt and GUI exit have unsurprising lifecycle behavior.

### 5.5 Stop

```text
<script> stop [--timeout SECONDS] [--force]
```

In headless mode, `stop` MUST request a graceful daemon shutdown. If a TCL command
is active, the default behavior MUST refuse to interrupt it, report the current
command and elapsed time, and exit non-zero. `--force` MAY escalate through
SIGTERM and then runtime kill after the specified grace period, and MUST clearly
state that the in-flight result may be lost.

In GUI mode, `stop` MUST request normal container termination. Stopping an absent
container SHOULD be idempotent. Removing the container after exit MAY be the
default, but persistent workspace files MUST never be deleted by `stop`.

## 6. Persistent Vivado TCL execution

### 6.1 Execute a command

```text
<script> exec [--timeout SECONDS] [--file TCL_FILE] [--] [TCL ...]
```

Exactly one of inline TCL or `--file` MUST be supplied. Inline arguments MUST be
joined with a single space only after argv parsing; callers SHOULD quote a full
TCL command as one argument. `--file` MUST read a host workspace file and submit
its complete content as one TCL operation. It does not grant shell execution.

The command MUST be sent to the sole persistent headless session. Calls in GUI
mode or without a running headless container MUST fail clearly. Commands MUST be
serialized: at most one command may own the Vivado interpreter. If another is in
flight, a new call MUST fail promptly with status `busy`, current command,
dispatch time, and last PTY-read time; it MUST NOT queue silently.

The daemon MUST own Vivado through a PTY and read until the exact Vivado prompt
returns or the process exits. A command timeout is client-side only. On timeout:

- the client MUST disconnect and exit `3`;
- the daemon MUST continue the command without interruption;
- the eventual result MUST be written atomically as the latest result; and
- the message MUST direct the caller to `<script> show result`.

Client exit, SSH loss, cancellation, or broken pipe MUST NOT cancel the Vivado
operation. Failure to deliver a completed response to the original client MUST
NOT crash the daemon.

### 6.2 Result semantics

```text
<script> show result [--json]
```

The latest completed result MUST contain at least:

```json
{
  "command": "get_projects",
  "started_at": "ISO-8601 timestamp",
  "finished_at": "ISO-8601 timestamp",
  "output": "filtered text",
  "errors": [],
  "had_errors": false,
  "truncated": false
}
```

Dispatching a new command MUST first replace the previous result with a durable
`no completed command` marker, so an in-flight command can never expose stale
output. Writes MUST use temporary-file plus atomic rename semantics. If no result
exists, `show result` MUST exit `2`.

Filtered result output MUST be capped at 1 MiB. Truncation MUST be explicit in
both metadata and text, using the marker `[output truncated at 1048576 bytes]`.
The raw log MUST remain uncapped by this rule.

### 6.3 Exit status

`exec` and `show result` MUST return:

| Code | Meaning |
|---:|---|
| `0` | Command completed without surfaced error severity |
| `1` | Vivado error/critical warning, busy state, crash, or protocol failure |
| `2` | CLI usage error or no completed result, as identified on stderr |
| `3` | Client wait timed out; command continues |

Vivado's process death MUST be distinguished in stderr from a TCL-reported error.

## 7. Output filtering and raw logging

All raw Vivado output, including startup, project opening, command headers, and
suppressed messages, MUST be appended to a timestamped session log. User-facing
command output MUST use a stateful line filter:

- `INFO:` is suppressed with its continuation lines.
- `CRITICAL WARNING:` and `ERROR:` are retained with continuations and surfaced
  as errors.
- `WARNING:` behavior depends on the optional workspace `elfws.yaml` file.
- Ordinary command output is retained and resets filter state.
- Indented lines and `Resolution:` belong to the preceding message decision.

With no suppression file, ordinary warnings MUST be suppressed. With a valid
suppression file, warning message IDs present in it MUST be suppressed and IDs
absent from it MUST be retained. Warnings without an ID MUST be suppressed. The
file MUST be reloaded per command so edits do not require a restart. A parse
failure MUST be recorded in the raw log and safely fall back to blanket warning
suppression; it MUST NOT crash the session.

## 8. Status and observability

### 8.1 Summary state

```text
<script> show status [--json]
<script> show metadata [--json]
<script> show health [--json]
```

`show status` MUST report container mode and lifecycle. For a headless session it
MUST additionally report:

- state: `starting`, `idle`, `busy`, `crashed`, `stopped`, `unreachable`, or
  `unknown`;
- current command or null;
- command runtime;
- time since the last PTY read;
- live non-zombie descendant count;
- aggregate descendant CPU percentage, where 100% is one full core;
- aggregate descendant RSS;
- timestamps of startup and latest health sampling.

An apparently live metadata file with an unreachable control endpoint MUST be
reported as `unreachable`; a read-only status command MUST NOT rewrite it as
`crashed`. Health sampling SHOULD occur every ten seconds and SHOULD sample CPU
over approximately one second. Process races and missing `/proc` entries MUST be
tolerated.

`show metadata` exposes raw lifecycle metadata. `show health` exposes the latest
process-tree sample and exits `2` when no sample exists yet.

Large PTY-idle time with high descendant CPU indicates a quiet active phase;
large PTY-idle time with no descendants and zero CPU indicates a likely wedge.
The CLI SHOULD explain this heuristic in help text but MUST NOT automatically
kill a session based on it.

### 8.2 Logs

```text
<script> logs [--tail N] [--follow]
```

Without `--follow`, the command MUST print a finite snapshot and exit. `--tail`
defaults to 50. With `--follow`, it MAY stream until interrupted. Log access MUST
read the host/runtime artifact directly and MUST NOT submit `exec cat`, `exec
tail`, or any equivalent TCL to Vivado.

### 8.3 Diagnostics

```text
<script> diagnose <last|metadata|health|inspect|logs|ps|wchan|fionread|fdtable> [options]
```

Every diagnostic MUST be read-only and based exclusively on session artifacts,
runtime inspection, and procfs. It MUST NOT open the daemon's command socket,
read from its PTY, or dispatch TCL. This separation prevents diagnostic output
containing `Vivado% ` from satisfying the daemon's own prompt matcher.

Required behavior:

| Probe | Result |
|---|---|
| `last` | Latest completed result |
| `metadata` | Raw session metadata |
| `health` | Latest process-tree health sample |
| `inspect` | Composite metadata, health, result, 50-line log tail, sample time |
| `logs --tail N` | Finite raw-log tail |
| `ps` | Vivado process tree with PID, state, sampled CPU%, and RSS KiB |
| `wchan` | Kernel wait channels for supervisor and Vivado |
| `fionread` | Non-destructive queued-byte counts for supervisor PTY FDs |
| `fdtable` | Supervisor file-descriptor numbers and resolved targets |

All probes except textual `logs` MUST support valid UTF-8 JSON. Missing optional
health/result data in `inspect --json` MUST be represented by `null`, not by
invalid JSON or a fabricated empty sample.

## 9. Integrated Vitis and companion tool workflow

### 9.1 General contract

```text
<script> run <xsct|xsdb|bootgen|dtc> [tool arguments ...]
```

Vitis and companion tools MUST be first-class operations in the same managed
Vivado/Vitis workflow. A `run` operation requires a running headless workflow
container; it MUST fail clearly when the container or workflow session is absent
and MUST NOT silently create an unrelated tool-only container.

Each operation MUST be registered in workflow metadata before execution and
MUST record its tool name, argv, working directory, start time, completion time,
state, and exit status. `show status` and `diagnose inspect` MUST include an
active tool operation and the latest completed tool operation. Tool output MUST
be included in the workflow's raw audit log, with timestamped operation headers,
while remaining available through the live stdout and stderr streams.

The tool runs as a child process within the managed container and shares the
workspace, UID/GID identity, pinned Xilinx environment, hardware-server setting,
and container lifecycle used by Vivado. Stopping or losing the container MUST
terminate the tool operation; an operation MUST never outlive its workflow.

The implementation MUST define one workflow scheduler for TCL and tool
operations. It SHOULD allow a tool operation to coexist with Vivado only when
the tool does not contend for exclusive resources such as a project lock,
workspace output, cable target, or vendor workspace. Otherwise it MUST reject
the operation as busy or serialize it explicitly. It MUST NOT start concurrent
work merely because the underlying executable is a separate process.

Tool argv, stdin, stdout, stderr, signals, and exit status MUST be preserved.
There MUST be no Vivado/TCL error translation and no wrapper text added to tool
stdout or stderr after successful dispatch.

### 9.2 `xsct`

```text
<script> run xsct <tclfile> [args ...]
```

The TCL file is required; omission exits `2`. The Vitis 2024.2 environment MUST
be initialized using the vendor settings script or a behaviorally equivalent
environment. The working directory MUST become the TCL file's directory, and the
tool MUST receive that file plus all remaining arguments verbatim. Relative
`source` statements must therefore resolve from the script directory.

### 9.3 `xsdb`

```text
<script> run xsdb [args ...]
```

The Vitis environment MUST be initialized, arguments preserved, and working
directory inherited from the mounted workspace unless the tool itself changes it.

### 9.4 `bootgen` and `dtc`

```text
<script> run bootgen [args ...]
<script> run dtc [args ...]
```

These commands MUST use the binaries supplied with the pinned Vivado/Vitis image
and preserve argv and exit code. They do not require Vitis environment setup if
the image makes them directly runnable. The interface MUST not depend on their
absolute installation paths.

## 10. Hardware-server integration

`<script> start ... --jtag-host` enables host-brokered JTAG. The URL resolution order
MUST be inline `--jtag-host=HOST:PORT`, configured/environment value, then
`host.docker.internal:3121`. The ambiguous two-token form `--jtag-host URL` MUST
be rejected. GUI mode SHOULD imply `--jtag-host`; explicitly supplying both MUST
be idempotent.

When enabled, the launcher MUST make `host.docker.internal` resolve to the host
gateway and pass the resolved URL into the application environment. Before
launch, it SHOULD probe TCP reachability. For the default host-gateway URL the
host-side probe target is `localhost:3121`. Failure MUST produce an actionable
warning mentioning Vivado Lab Edition/LabTools and `hw_server -d`, but MUST NOT
prevent startup because the server may be started later.

The system MUST NOT automatically connect to hardware. A caller chooses when to
execute the equivalent of:

```tcl
connect_hw_server -url $::env(VIVADO_HW_SERVER_URL)
open_hw_target
```

No mode may mount `/dev/bus/usb` or `/run/udev`, add USB device-cgroup rules, or
install host udev rules as part of this feature.

## 11. Constrained command transport

When SSH is enabled, the unprivileged tool account MUST use public-key-only
authentication and MUST be constrained to the public CLI dispatcher. The
dispatcher MUST preserve `SSH_ORIGINAL_COMMAND` as data and parse it without
`eval`. An empty remote command MUST show help or return a clear usage error; it
MUST NOT accidentally dispatch empty TCL.

Operational inspection MUST use the documented status, log, and read-only
diagnostic surfaces.

## 12. Runtime hardening and Vivado compatibility

The image MUST prevent the known Vivado 2024.2 WebTalk/licensing crash caused by
host-device enumeration through `libudev`. A compatibility library that provides
the complete public `udev_*` symbol surface MAY be used. If used, it MUST have the
correct SONAME and MUST return a safe empty enumeration.

The compatibility search path MUST apply only to the Vivado process tree in both
headless and GUI modes. SSH command sessions, diagnostic tools,
and unrelated utilities MUST continue to resolve the system `libudev`. The fix
MUST be verified against at least `open_project` and run launch; neither may abort
in the licensing stack.

The CLI MUST report the image/tool version in `--version` output and SHOULD reject
or warn on a container whose protocol version is incompatible with the host CLI.

## 13. Session artifacts and protocol

The implementation MAY choose artifact locations, but the headless session MUST
durably maintain equivalents of:

| Artifact | Required content |
|---|---|
| metadata | Session ID, project/workspace, timestamps, state, current command |
| raw log | Unfiltered startup and command output with command headers |
| latest result | Atomic latched result or no-result marker |
| health | Latest atomic process-tree sample |
| control endpoint | Local-only command transport |
| process identity | Supervisor and Vivado process IDs or runtime equivalents |

Internal communication MUST use a filesystem-scoped local endpoint such as a
Unix domain socket. It MUST NOT bind a TCP port. The protocol MUST frame requests
and responses unambiguously, reject malformed or oversized requests, and ensure
that only the owning tool account and the container runtime can access it.

Session artifact discovery MUST be independent of caller working directory in
container mode. The runtime/entrypoint MUST provide a single explicit session
root. Missing or inaccessible session storage MUST fail startup; it MUST not
silently fall back to an arbitrary directory.

## 14. Machine-readable output

Commands supporting `--json` MUST write exactly one valid JSON value to stdout.
Informational messages and warnings MUST go to stderr. JSON keys MUST remain
stable within a protocol major version. Timestamps MUST be ISO 8601 and SHOULD
include an offset or `Z`. Durations SHOULD also be exposed as numeric seconds,
even if human output uses forms such as `12m02s`.

Paths reported to the host MUST use host-visible paths where a mapping exists.
Internal container paths MAY be included in separate explicitly named fields.

## 15. Acceptance scenarios

An implementation is conformant only if automated tests cover at least:

1. Headless startup with and without a project, readiness gating, duplicate
   start, graceful stop, and startup failure.
2. GUI startup for X11/XWayland/WSLg, no SSH/session artifacts, project opening,
   host file ownership, and exit on window close.
3. UID/GID mismatch, matching-ID fast path, and missing-ID failure.
4. PID 1 reaping, signal forwarding, and absence of accumulating zombies.
5. Successful TCL execution, TCL error, busy rejection, client timeout followed
   by result retrieval, client disconnect, Vivado crash, and 1 MiB truncation.
6. Stateful filtering, warning suppression reload, malformed suppression file,
   and preservation of all raw output.
7. Every show/diagnostic JSON shape, absent artifacts, procfs races, and proof
   that diagnostics neither open the control socket nor consume PTY bytes.
8. `xsct`, `xsdb`, `bootgen`, and `dtc` workflow registration,
   argv/stream/exit-code preservation, scheduler resource conflicts, audit-log
   integration, and rejection when no workflow container exists.
9. JTAG URL precedence, default host mapping, failed non-blocking probe, and
   absence of USB/udev mounts.
10. SSH disabled without a key and constrained unprivileged SSH command access.
11. Vivado-only compatibility-library scoping and successful `open_project` plus
    synthesis/implementation run launch.
12. Argument paths containing spaces and shell metacharacters, demonstrating that
    no `eval`-based reparsing occurs.

## 16. Interface transition principles

The desired interface replaces implicit TCL fallthrough with explicit `exec`,
groups state reads beneath `show`, groups read-only inspection beneath
`diagnose`, and groups Xilinx tool operations beneath `run`. Compatibility
aliases MAY be offered for one transition period, but MUST be documented as
deprecated and MUST NOT restore wildcard TCL fallthrough.

