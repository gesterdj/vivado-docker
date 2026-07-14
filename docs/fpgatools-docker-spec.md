# fpgatools Docker — Consolidated Specification

Normative requirements consolidated from the OpenSpec specs (`openspec/specs/`), all
OpenSpec change deltas (`openspec/changes/`), and `Dockerfile-vivado-25.2`.

Requirement keywords (SHALL, SHALL NOT, MUST) follow RFC 2119 usage. Each requirement
carries verification scenarios in **WHEN/THEN** form.

Capabilities covered:

| # | Capability | Scope |
|---|---|---|
| 1 | Vivado 2025.2 base image | `Dockerfile-vivado-25.2` build contract |
| 2 | Vivado session container | `Dockerfile-vivado-session` image + entrypoint + `vcli` |
| 3 | Container init | PID 1 init contract (tini) |
| 4 | Container GUI mode | Interactive `vivado -mode gui` operation |
| 5 | Container UID alignment | Runtime `HOST_UID`/`HOST_GID` passthrough |
| 6 | Vivado libudev stub | WebTalk/FlexNet libudev crash mitigation |
| 7 | JTAG host broker | Host-side `hw_server` over TCP |

---

## 1. Vivado 2025.2 base image (`Dockerfile-vivado-25.2`)

### Requirement: Base platform and system packages

The 2025.2 base image SHALL build from `ubuntu:22.04` with `DEBIAN_FRONTEND=noninteractive`
and SHALL install, via apt with retry (`Acquire::Retries=3`) and `--no-install-recommends`,
the toolchain and runtime libraries required by the Vivado/Vitis installer and tools:
`build-essential`, `tcl-dev`, `libtinfo5`, `libncurses5`, `lsb-release`, `xz-utils`,
`unzip`, `curl`, `file`, `git`, `wget`, `locales`, X11/GTK client libraries (`libxext6`,
`libxft2`, `libxi6`, `libxrender1`, `libxtst6`, `libgtk2.0-0`, `libsm6`, `libice6`,
`libglib2.0-0`, `fontconfig`, `libpng16-16`), `udev`, and `expect`. Apt caches SHALL be
cleaned (`apt-get clean`, removal of `/var/lib/apt/lists/*`) in the same layer.

#### Scenario: Installer prerequisites present
- **WHEN** the image is built
- **THEN** the apt layer completes with retries enabled, no recommended packages, and no
  residual apt list cache.

### Requirement: UTF-8 locale

The image SHALL generate the `en_US.UTF-8` locale and export `LANG=en_US.UTF-8` and
`LC_ALL=en_US.UTF-8`.

#### Scenario: Locale active
- **WHEN** a container from the image runs `locale`
- **THEN** `LANG` and `LC_ALL` report `en_US.UTF-8`.

### Requirement: Authenticated batch install of Vivado/Vitis 2025.2

The image SHALL install Vivado/Vitis 2025.2 in unattended batch mode:

- Xilinx credentials SHALL be provided as build args `XILINX_EMAIL` and `XILINX_PASSWORD`
  (`docker build --build-arg XILINX_EMAIL=... --build-arg XILINX_PASSWORD=...`) and
  exported into the build environment.
- The build SHALL copy `xsetup_config_25.txt` (as `xsetup_config.txt`) and
  `auth_token_25.2.expect` into `/tmp/xilinx`.
- The pre-unpacked local installer directory `./Xilinx` SHALL be provided via a BuildKit
  bind mount (`--mount=type=bind,source=./Xilinx,target=/opt/xilinx_installer`), not
  copied into the build context.
- The build SHALL first generate an auth token by running `auth_token_25.2.expect`, then
  run `/opt/xilinx_installer/2025.2/xsetup --batch Install
  --agree XilinxEULA,3rdPartyEULA -c xsetup_config.txt`.
- Post-install, the build SHALL remove `/tmp/xilinx` and `/opt/Xilinx/Downloads`.

#### Scenario: Batch install succeeds without interaction
- **WHEN** the image is built with valid `XILINX_EMAIL`/`XILINX_PASSWORD` and a populated
  `./Xilinx/2025.2` installer tree
- **THEN** the expect script produces the auth token, xsetup completes in batch mode with
  both EULAs agreed, and neither the installer staging dir nor the Downloads cache remain
  in the final layer.

### Requirement: Tool paths and default command

The image SHALL set `VIVADO_PATH=/opt/Xilinx/Vivado/2025.2` and
`VITIS_PATH=/opt/Xilinx/Vitis/2025.2`, prepend `$VIVADO_PATH/bin` and `$VITIS_PATH/bin`
to `PATH`, set the working directory to `/workspace`, and default to
`CMD ["vivado", "-mode", "batch"]`.

#### Scenario: Tools on PATH
- **WHEN** a container is started from the image with no command
- **THEN** `vivado -mode batch` launches from `/workspace`, and both `vivado` and Vitis
  binaries resolve on `PATH`.

---

## 2. Vivado session container

### Requirement: Container image is built from vivado-session Dockerfile

The system SHALL provide a `Dockerfile-vivado-session` that extends the
`vivado-vitis-24.2` base image, adds `python3`, `pexpect`, `openssh-server`, `gosu`, and
the `vivado-cli` toolset, and sets `VIVADO_SESSIONS_ROOT=/run/vivado_sessions`.

#### Scenario: Image builds successfully
- **WHEN** `docker build -f Dockerfile-vivado-session -t vivado-session:latest .` is run
- **THEN** the build completes without error and the resulting image contains
  `vivado_start`, `vivado_exec`, `vivado_sessions`, and `vcli` on PATH.

### Requirement: Entrypoint performs privilege-split via gosu

The entrypoint.sh SHALL run initially as root, perform privileged setup, then re-exec
itself as the `vivado` user via `exec gosu vivado "$0" "$@"` before launching the Vivado
session.

#### Scenario: Root section completes before user section
- **WHEN** the container starts
- **THEN** the root section runs first (SSH setup, directory creation), and the vivado
  user section runs only after the `gosu` re-exec.

#### Scenario: Vivado process becomes the foreground process
- **WHEN** the entrypoint re-execs as the vivado user
- **THEN** `exec vcli start` replaces the shell process, making Vivado the direct
  descendant of PID 1.

### Requirement: SSH access is opt-in via /run/pubkey bind-mount

The entrypoint SHALL install `/run/pubkey` as the `vivado` user's `authorized_keys` and
start `sshd` ONLY when `/run/pubkey` exists at container startup. If `/run/pubkey` is
absent, sshd SHALL NOT be started.

#### Scenario: SSH enabled when pubkey is present
- **WHEN** the container starts with `/run/pubkey` bind-mounted
- **THEN** the key is installed to `/home/vivado/.ssh/authorized_keys` and `sshd` is
  started before the privilege drop.

#### Scenario: SSH not started when pubkey is absent
- **WHEN** the container starts without `/run/pubkey`
- **THEN** no `sshd` process is started and the container proceeds to the vivado user
  section.

### Requirement: Vivado sessions directory is initialized by entrypoint

The entrypoint SHALL create `/run/vivado_sessions/` (if absent) and set ownership to
`vivado:vivado` before the privilege drop.

#### Scenario: Directory created and owned correctly
- **WHEN** the container starts as root
- **THEN** `/run/vivado_sessions/` exists and is owned by the `vivado` user before the
  gosu re-exec.

### Requirement: vcli dispatches container session commands

The `vcli` script SHALL dispatch the following subcommands, always targeting the session
named "main":

- `vcli start [--project PATH]` → `vivado_start --name main [--project PATH] --foreground`
- `vcli stop` → `vivado_sessions stop main`
- `vcli logs` → `vivado_sessions logs main`
- `vcli list` → `vivado_sessions list`
- `vcli <other>` → `vivado_exec --session main "<other>"` (pass-through TCL)

#### Scenario: vcli start launches foreground session
- **WHEN** `vcli start` is called (optionally with `--project PATH`)
- **THEN** `vivado_start --name main --foreground` is invoked (with `--project PATH` if
  provided), blocking until Vivado exits.

#### Scenario: vcli stop terminates the session
- **WHEN** `vcli stop` is called from an SSH session
- **THEN** `vivado_sessions stop main` is invoked, terminating the running session.

#### Scenario: vcli passes unknown args as TCL
- **WHEN** `vcli "open_project /path/to/foo.xpr"` is called
- **THEN** `vivado_exec --session main "open_project /path/to/foo.xpr"` is invoked.

### Requirement: VIVADO_PROJECT env var auto-opens a project at startup

The entrypoint SHALL pass `--project "$VIVADO_PROJECT"` to `vcli start` when the
`VIVADO_PROJECT` environment variable is set. When unset, no `--project` argument SHALL
be passed.

#### Scenario: Project opened automatically when env var is set
- **WHEN** the container starts with `VIVADO_PROJECT=/data/project/foo.xpr`
- **THEN** `vcli start --project /data/project/foo.xpr` is called.

#### Scenario: No project argument when env var is unset
- **WHEN** the container starts without `VIVADO_PROJECT`
- **THEN** `vcli start` is called with no `--project` argument.

### Requirement: Container lifetime equals Vivado session lifetime

The container SHALL exit when the Vivado session process exits, whether that is due to
normal termination, a `vcli stop` command, or a crash.

#### Scenario: Container exits on Vivado exit
- **WHEN** the Vivado process terminates (any reason)
- **THEN** the container exits with the same exit code.

### Requirement: run-remote.sh references the new image name

`run-remote.sh` SHALL use `vivado-session:latest` as the image name and SHALL NOT
reference tmux in its output.

#### Scenario: Script uses correct image name
- **WHEN** `run-remote.sh` is executed
- **THEN** the `docker run` command references `vivado-session:latest`.

---

## 3. Container init

Defines the container's PID 1 init contract: reaping zombie processes re-parented to
PID 1, forwarding termination signals to the application daemon, and keeping the
container's lifecycle coupled to the daemon.

### Requirement: PID 1 is a zombie-reaping init process

The vivado-session container image SHALL use `tini` (installed at `/usr/bin/tini`) as
PID 1 via the Dockerfile `ENTRYPOINT`. The ENTRYPOINT SHALL be specified in exec form as
`["/usr/bin/tini", "--", "/entrypoint.sh"]`.

tini at PID 1 SHALL reap any process re-parented to PID 1, so that the running container
does not accumulate `<defunct>` (zombie) entries from orphaned grandchildren of Vivado,
sshd, or any other child of the application daemon.

#### Scenario: tini is PID 1 inside the running container
- **WHEN** a container built from the image is started and inspected with
  `ps -o pid,comm`
- **THEN** PID 1 is `tini` and the application daemon (`vivado_start` / `python3`) is a
  child of PID 1, not PID 1 itself.

#### Scenario: Orphaned grandchild is reaped
- **WHEN** any process inside the container terminates while its parent has already
  exited, leaving it re-parented to PID 1
- **THEN** the process is reaped promptly by tini and does not appear as `<defunct>` in a
  subsequent `ps` listing.

#### Scenario: Long-lived container does not accumulate zombies
- **WHEN** a container has been running for an extended period during which Vivado has
  executed many commands, including ones that spawn short-lived subprocesses
- **THEN** the count of processes in state `Z` (zombie) inside the container remains at
  or near zero, rather than growing monotonically.

### Requirement: PID 1 forwards termination signals to the application

tini at PID 1 SHALL forward SIGTERM and SIGINT to its direct child (the entrypoint script
and, transitively after exec, the application daemon). Container shutdown initiated by
`docker stop` SHALL therefore reach the application via standard signal delivery rather
than via the post-grace-period SIGKILL.

#### Scenario: docker stop terminates promptly
- **WHEN** `docker stop <container>` is invoked on a running container with an idle
  Vivado session
- **THEN** the container exits within the default grace period (10 s) without requiring
  SIGKILL escalation, because tini forwards SIGTERM to the daemon and the daemon's
  shutdown path runs to completion.

### Requirement: Container lifecycle remains coupled to the daemon

The introduction of tini SHALL NOT decouple the container's lifecycle from the daemon's
lifecycle. When the daemon (tini's direct child) exits — whether through `vcli stop`, a
crash, or any other path — tini SHALL exit with the child's status and the container
SHALL terminate. This preserves the existing one-container-per-session operational
contract.

#### Scenario: Daemon exit ends the container
- **WHEN** the application daemon inside a tini-led container exits with any status
- **THEN** tini exits with the same status, the container exits, and a subsequent
  `docker ps` shows the container as stopped.

---

## 4. Container GUI mode

A GUI mode for the vivado-session container, mutually exclusive with the headless TCL
daemon mode. Mode selection happens at container start; the running mode is fixed for
the container's lifetime.

### Requirement: Mode selection at container start

The container SHALL select between headless TCL daemon mode and GUI mode by inspecting
the `VIVADO_MODE` environment variable in its entrypoint. When `VIVADO_MODE=gui`, the
container SHALL run in GUI mode. When unset or set to any other value (including `tcl`),
the container SHALL run in headless mode. The selected mode SHALL be fixed for the
lifetime of the container — there SHALL be no runtime switch between modes.

#### Scenario: GUI mode selected
- **WHEN** the container is started with `VIVADO_MODE=gui`
- **THEN** the entrypoint exec's into `vivado -mode gui` (after UID alignment and gosu
  pivot) without starting sshd, without creating `/run/vivado_sessions`, and without
  invoking the TCL daemon.

#### Scenario: Headless mode selected by default
- **WHEN** the container is started with `VIVADO_MODE` unset
- **THEN** the entrypoint follows the existing headless path: starts sshd if a pubkey is
  present, creates `/run/vivado_sessions`, and exec's the TCL daemon via `vcli start`.

### Requirement: GUI mode runs Vivado as PID 2

In GUI mode, the entrypoint SHALL `exec` into Vivado (via `gosu vivado`) so that Vivado
becomes PID 2 (the direct child of tini at PID 1). No supervisor or wrapper SHALL stay
resident between tini and Vivado.

#### Scenario: GUI process tree
- **WHEN** a GUI-mode container is running and inspected with `ps -ef`
- **THEN** PID 1 is `tini`, PID 2 is `vivado` running in `-mode gui`, and there are no
  other long-lived processes (no sshd, no `vivado_start`, no `_daemon.py`).

### Requirement: No SSH, no daemon, no diagnostic surface in GUI mode

In GUI mode, the container SHALL NOT start sshd, SHALL NOT start the TCL daemon, SHALL
NOT create `/run/vivado_sessions`, and SHALL NOT expose any `vcli` or `vcli diag`
interface. The launcher SHALL NOT write `.vivado-session` to the host project directory
in GUI mode.

#### Scenario: No SSH listener
- **WHEN** a GUI-mode container is running and the host attempts `ss -tlnp` or
  `nc -zv localhost <port>` on any port the launcher might have mapped
- **THEN** no port is mapped and no SSH listener exists inside the container.

#### Scenario: No .vivado-session written in GUI mode
- **WHEN** the launcher is invoked with `--gui` in a project directory
- **THEN** no `.vivado-session` file is written, regardless of whether one existed
  previously.

### Requirement: X11 transport via docker mounts

In GUI mode, the container SHALL reach the host's X11 server through docker bind mounts
of the host's X11 Unix socket directory and X authority. Specifically:

- The launcher SHALL bind-mount `/tmp/.X11-unix` from the host to `/tmp/.X11-unix` in
  the container.
- The launcher SHALL bind-mount the host's `$XAUTHORITY` cookie file to
  `/run/host-xauth` in the container and set `XAUTHORITY=/run/host-xauth` in the
  container's environment.
- The launcher SHALL forward the host's `DISPLAY` environment variable to the container.

SSH-tunneled X11 forwarding SHALL NOT be used.

#### Scenario: Vivado GUI renders on the host display
- **WHEN** the launcher is invoked with `--gui` on a Linux host with an X11 or XWayland
  server running
- **THEN** Vivado opens a window on the operator's desktop within the X server reached
  through the mounted `/tmp/.X11-unix` socket.

### Requirement: Host-profile detection and override

The launcher SHALL autodetect the host's X11 environment and apply the appropriate
docker mounts and environment variables. The autodetected profile SHALL be one of `wsl`,
`wayland`, or `x11`. The launcher SHALL accept a `VIVADO_GUI_PROFILE` environment
variable that, when set to one of those three values, overrides autodetection.

Detection rules:
- `wsl` SHALL be selected when `/proc/version` contains `microsoft` or `WSL`
  (case-insensitive).
- `wayland` SHALL be selected when `WAYLAND_DISPLAY` is set in the launcher's
  environment.
- `x11` SHALL be selected otherwise (default fallback).

#### Scenario: WSL2 with WSLg autodetected
- **WHEN** the launcher is invoked with `--gui` from a WSL2 shell whose `/proc/version`
  reports a Microsoft kernel
- **THEN** the launcher sets profile `wsl`, mounts `/mnt/wslg:/mnt/wslg` and
  `/tmp/.X11-unix:/tmp/.X11-unix`, and sets `DISPLAY`, `WAYLAND_DISPLAY`, and
  `XDG_RUNTIME_DIR` env in the container.

#### Scenario: Operator overrides detection
- **WHEN** the launcher is invoked with
  `VIVADO_GUI_PROFILE=x11 ./run-vivado-cli.sh --gui` on a WSL2 host
- **THEN** the launcher applies the `x11` profile mounts and env regardless of
  `/proc/version` content.

### Requirement: Container lifecycle ends with Vivado GUI

The container's lifecycle in GUI mode SHALL be coupled to the Vivado GUI process. When
the user closes the Vivado window or Vivado otherwise terminates, the container SHALL
exit promptly.

#### Scenario: Closing Vivado ends the container
- **WHEN** the operator closes the main Vivado window in a GUI-mode container
- **THEN** Vivado exits, tini exits with Vivado's status, and the container is no longer
  listed by `docker ps` within a few seconds.

### Requirement: Project mount path identical to headless mode

In GUI mode, the launcher SHALL bind-mount the operator's current working directory to
`/project` in the container, identical to headless mode. If `VIVADO_PROJECT` is set, the
entrypoint SHALL pass it as `--project` to Vivado; if unset, Vivado opens to its blank
welcome screen.

#### Scenario: VIVADO_PROJECT opens specified xpr
- **WHEN** the launcher is invoked with `--gui` and `VIVADO_PROJECT=/project/foo.xpr`
- **THEN** Vivado launches and immediately opens `/project/foo.xpr`.

#### Scenario: No project specified
- **WHEN** the launcher is invoked with `--gui` and no `VIVADO_PROJECT`
- **THEN** Vivado launches with its standard welcome screen; the operator can navigate
  to `/project/...` via the GUI.

### Requirement: GUI mode inherits UID alignment

GUI mode SHALL apply the same `HOST_UID`/`HOST_GID` alignment as headless mode (see
capability *Container UID alignment*). Files written by Vivado into the bind-mounted
`/project` SHALL appear on the host filesystem owned by the host operator's UID and GID.

#### Scenario: Saved design files owned by host operator
- **WHEN** the operator, in a GUI-mode container, saves a design change (e.g., adds an
  IP and writes the project)
- **THEN** the resulting modifications to files under `/project` on the host filesystem
  are owned by `$(id -u):$(id -g)` and editable without privilege escalation.

---

## 5. Container UID alignment

Makes the container's `vivado` UID/GID track the host operator's UID/GID at container
start rather than at image build time, so bind-mounted project files never need
`sudo chown`.

### Requirement: Container vivado user UID/GID match host operator

The container's `vivado` user SHALL be runtime-aligned to the host operator's UID and
GID before any application process starts. The launcher SHALL pass `HOST_UID` and
`HOST_GID` environment variables to `docker run`; the entrypoint, running as root, SHALL
rewrite the in-container `vivado` user's UID to `HOST_UID` and GID to `HOST_GID`, and
SHALL recursively chown `/home/vivado` to the new UID:GID pair.

The rewrite SHALL be conditional: when the in-container `vivado` UID already equals
`HOST_UID`, the entrypoint SHALL skip `usermod`, `groupmod`, and the chown to avoid
unnecessary work on hosts where the build-time UID happens to match.

#### Scenario: Host UID differs from build-time UID
- **WHEN** the launcher starts a container on a host where `$(id -u)` is `1001`
  (different from the image's build-time `vivado` UID of `1000`)
- **THEN** the entrypoint's root branch rewrites the `vivado` account to UID `1001`, and
  `id vivado` inside the running container reports `1001`.

#### Scenario: Host UID matches build-time UID
- **WHEN** the launcher starts a container on a host where `$(id -u)` is `1000` (same as
  the image's build-time `vivado` UID)
- **THEN** the entrypoint detects the match and skips `usermod`, `groupmod`, and the
  chown of `/home/vivado`.

### Requirement: Required HOST_UID and HOST_GID environment

The entrypoint SHALL require both `HOST_UID` and `HOST_GID` to be set in its root branch
and SHALL fail with a clear error message if either is missing or empty. Silent fallback
to the image's build-time `vivado` UID SHALL NOT occur.

#### Scenario: Launcher omits HOST_UID
- **WHEN** the container is started without `HOST_UID` set in the environment
- **THEN** the entrypoint exits non-zero with an error naming the missing variable,
  before any application process is started.

### Requirement: Bind-mounted project files owned by host operator

Files created by Vivado (running as the retargeted `vivado` user) inside the
bind-mounted `/project` directory SHALL appear on the host filesystem owned by the host
operator's UID and GID, requiring no `sudo chown` to edit afterward.

#### Scenario: Vivado writes to /project
- **WHEN** Vivado, executing inside a UID-aligned container, writes a file under
  `/project` (such as a synthesis log, runs directory, or bitstream output)
- **THEN** on the host, that file is owned by `$(id -u):$(id -g)` and is directly
  readable and writable by the host operator's user account without privilege
  escalation.

### Requirement: System paths retain image ownership

The UID rewrite SHALL be scoped to `/home/vivado` and the `vivado` user's `/etc/passwd`
/ `/etc/group` entries. System directories — including `/opt/vivado-cli/`,
`/usr/local/bin/`, `/etc/`, and `/run` — SHALL retain their build-time ownership and
SHALL NOT be chowned at startup.

#### Scenario: Application code remains readable, not retargeted
- **WHEN** the entrypoint completes the UID alignment for a host UID that differs from
  the build-time UID
- **THEN** `/opt/vivado-cli/` and other system paths are still owned by their original
  build-time UIDs, but the running `vivado` process (now at the host UID) can still read
  and execute them via standard mode bits.

---

## 6. Vivado libudev stub

Mitigates a deterministic Vivado crash (WebTalk → `libXil_lmgr11.so` FlexNet host
fingerprinting → `dlopen("libudev.so.1")` → `udev_enumerate_scan_devices` → glibc heap
abort) on `open_project` and `launch_runs`. The working mitigation is a full shadow
`libudev.so.1` reached via `LD_LIBRARY_PATH`; single-symbol `LD_PRELOAD` stubs are
bypassed by `dlopen + RTLD_LOCAL + dlsym`.

### Requirement: Stub libudev library shipped in the container image

The container image SHALL include a shadow `libudev.so.1` at the fixed path
`/opt/vivado-stubs/libudev.so.1`. The stub SHALL be built at image-build time from the
image's own real `libudev.so.1`, by enumerating all `T udev_*` exports and generating a
no-op implementation for each.

The stub SHALL have SONAME exactly `libudev.so.1`, so that `dlopen("libudev.so.1")`
resolves to the stub when `/opt/vivado-stubs/` precedes `/lib/x86_64-linux-gnu/` in the
dynamic-loader search path.

The stub SHALL export every public symbol present in the image's real libudev (currently
92 functions for the Ubuntu 22.04 libudev1 package), regardless of whether Vivado's
licensing code is known to call each. This insulates the image from any future Xilinx
code-path shift onto a different libudev symbol that would trip the same crash.

Each stub function body SHALL be `void* funcname() { return 0; }`, returning NULL (for
pointer returns) or 0 (for integer returns). The x86-64 calling convention makes the
prototype mismatch safe — caller-provided arguments are passed in registers and ignored
by the callee.

#### Scenario: Stub library exists and has correct SONAME
- **WHEN** the container is built and an inspection is performed on
  `/opt/vivado-stubs/libudev.so.1`
- **THEN** the file exists, `objdump -p` reports SONAME `libudev.so.1`, and `nm -D`
  lists all 92 public `udev_*` symbols as `T` (text, exported).

#### Scenario: Build toolchain remains in the image
- **WHEN** the container is inspected for the gcc compiler used to build the stub
- **THEN** `gcc` and the libc development headers are still present, allowing in-place
  rebuild of the stub if libudev is ever updated.

### Requirement: LD_LIBRARY_PATH prepends the stub directory for Vivado processes only

The container entrypoint SHALL prepend `/opt/vivado-stubs` to `LD_LIBRARY_PATH` such
that the path is active for Vivado (in both GUI and headless modes) but not for
processes that do not descend from the Vivado launch chain.

In GUI mode, the entrypoint's `gosu vivado env …` exec line SHALL include
`LD_LIBRARY_PATH=/opt/vivado-stubs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}` alongside any
other env vars passed to Vivado.

In headless mode, the entrypoint SHALL
`export LD_LIBRARY_PATH=/opt/vivado-stubs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}` before
exec-ing `vcli start`, so the Python daemon (and the Vivado process it spawns via
pexpect) inherit the path.

#### Scenario: Vivado process tree resolves dlopen("libudev.so.1") to the stub
- **WHEN** Vivado, launched in either mode, performs `dlopen("libudev.so.1")` via its
  licensing layer
- **THEN** the resolved file is `/opt/vivado-stubs/libudev.so.1`, and subsequent `dlsym`
  calls return the stub functions.

#### Scenario: SSH-logged-in user session does not see the stub path
- **WHEN** an operator opens an SSH session (via `vcli` ForceCommand or via
  `enable-root-shell` + `ssh root@<container>`)
- **THEN** `LD_LIBRARY_PATH` in that session does not contain `/opt/vivado-stubs`, and
  `udevadm info --export-db` (or any other libudev consumer) uses the real
  `/lib/x86_64-linux-gnu/libudev.so.1`.

### Requirement: WebTalk fingerprinting no longer crashes on open_project or launch_runs

With the stub active, Vivado's WebTalk fingerprinting code path (which calls
`libXil_lmgr11.so xilinxd_…` → `udev_enumerate_scan_devices`) SHALL receive a
successful, empty enumeration from the stub and SHALL NOT abort.

This SHALL hold for the operations that previously crashed:

- `open_project` on any project
- `launch_runs synth_1 -jobs N`
- `launch_runs impl_1 -jobs N`

#### Scenario: open_project no longer crashes
- **WHEN** Vivado opens any `.xpr` under the new image
- **THEN** no `hs_err_pid*.log` is produced, and Vivado proceeds to its normal post-open
  state (sources scanned, IP repositories refreshed).

#### Scenario: launch_runs synth_1 no longer crashes
- **WHEN** Vivado is instructed to `launch_runs synth_1 -jobs 4` on a previously
  crashing project
- **THEN** Vivado's parent process survives past the launch, the synth subprocess starts
  and writes to `runme.log`, and any subsequent error (e.g., real design-source errors)
  is reported through Vivado's normal error path, not via `abort()`.

### Requirement: Stub does not affect system libudev consumers in ad-hoc shells

The stub library SHALL be activated only via the `LD_LIBRARY_PATH` env set by the
entrypoint for the daemon process tree. System utilities that link against libudev
(e.g., `udevadm`, `lsblk`, `mount`, future udev-using diagnostics) and are invoked from
a fresh SSH session SHALL continue to use the real
`/lib/x86_64-linux-gnu/libudev.so.1`.

#### Scenario: udevadm continues to work from a debug shell
- **WHEN** a root operator (via `enable-root-shell` + `ssh root@<container>`) runs
  `udevadm info --export-db`
- **THEN** the command produces the host's udev database output, demonstrating that the
  real libudev is still resolvable in that environment.

---

## 7. JTAG host broker

Replaces in-container USB JTAG plumbing with a host-native `hw_server` (Xilinx
LabTools / Lab Edition) that the container's Vivado reaches over TCP.

### Requirement: --jtag-host launcher flag with optional URL override

The launcher SHALL accept a `--jtag-host[=URL]` flag. When passed without `=URL`, the
JTAG broker URL defaults to `host.docker.internal:3121`. When passed as
`--jtag-host=URL`, the operator-supplied URL is used verbatim. An exported
`VIVADO_HW_SERVER_URL` environment variable in the operator's shell SHALL serve as the
override path when `--jtag-host` is passed without an inline URL.

The space-separated form `--jtag-host URL` SHALL be rejected — `=URL` is the only
supported inline form, to keep argument parsing unambiguous against neighboring flags.

#### Scenario: --jtag-host defaults to host.docker.internal:3121
- **WHEN** `./run-vivado-cli.sh --jtag-host` is invoked with no other JTAG configuration
- **THEN** the docker run command includes
  `-e VIVADO_HW_SERVER_URL=host.docker.internal:3121` and
  `--add-host=host.docker.internal:host-gateway`.

#### Scenario: --jtag-host=URL overrides the default
- **WHEN** `./run-vivado-cli.sh --jtag-host=192.0.2.10:3121` is invoked
- **THEN** `VIVADO_HW_SERVER_URL=192.0.2.10:3121` is set in the container environment.

#### Scenario: Exported VIVADO_HW_SERVER_URL overrides the default
- **WHEN** the operator exports `VIVADO_HW_SERVER_URL=lab-broker.example.com:3121` and
  runs `./run-vivado-cli.sh --jtag-host`
- **THEN** that URL is propagated into the container; the default is not used.

### Requirement: --gui implies --jtag-host

When `--gui` is passed to the launcher, the JTAG host-broker wiring (env var +
`--add-host`) SHALL be applied automatically as if `--jtag-host` had also been passed.
Explicitly passing both flags SHALL behave identically to `--gui` alone.

`--jtag-host` without `--gui` SHALL remain valid for headless agent-driven flows that
need cable access.

#### Scenario: GUI mode wires up host broker automatically
- **WHEN** the launcher is invoked as `./run-vivado-cli.sh --gui`
- **THEN** `VIVADO_HW_SERVER_URL` is set in the container env and
  `--add-host=host.docker.internal:host-gateway` is applied, identical to what
  `--gui --jtag-host` would produce.

### Requirement: Container reaches host hw_server via host.docker.internal

The launcher SHALL pass `--add-host=host.docker.internal:host-gateway` whenever JTAG
host-broker mode is active. This SHALL make `host.docker.internal` resolve to the host's
bridge gateway from inside the container on both Linux (Docker Engine 20.10+) and
WSL2 / Docker Desktop, so that a Vivado TCL
`connect_hw_server -url host.docker.internal:3121` reaches the host's `hw_server`
listener.

#### Scenario: Container resolves host.docker.internal
- **WHEN** a container started with `--jtag-host` is inspected from inside
  (`getent hosts host.docker.internal`)
- **THEN** the name resolves to a routable IP corresponding to the host's bridge
  gateway.

#### Scenario: Other containers on vivado-net remain reachable
- **WHEN** the launcher is invoked with `--jtag-host` and the container joins the
  `vivado-net` network alongside other vivado-session containers
- **THEN** the sibling containers are still reachable by their container name through
  Docker's embedded DNS; the `--add-host` entry does not displace or interfere with
  bridge-network DNS.

### Requirement: Pre-launch hw_server reachability probe

When `--jtag-host` is active (explicitly or via `--gui`), the launcher SHALL probe the
chosen URL from the host's perspective before invoking `docker run`. For the default
`host.docker.internal:3121`, the launcher SHALL probe `localhost:3121`. For
operator-supplied URLs pointing elsewhere, the launcher SHALL probe that address
directly.

If the probe fails (TCP connect refused or times out), the launcher SHALL print a
multi-line warning naming the URL, the LabTools / Vivado Lab Edition install
requirement, the `hw_server -d` startup command, and the note that `connect_hw_server`
inside Vivado will fail until `hw_server` is started. The launcher SHALL NOT gate launch
on the probe result — `hw_server` may be started later in the session.

#### Scenario: hw_server reachable
- **WHEN** the launcher is invoked with `--jtag-host` and `hw_server` is listening on
  the configured port
- **THEN** the probe succeeds silently and no warning is emitted.

#### Scenario: hw_server not running
- **WHEN** the launcher is invoked with `--jtag-host` and no listener is on the
  configured port
- **THEN** the launcher prints a warning naming the URL and the LabTools /
  `hw_server -d` setup steps, and continues with the docker run regardless.

### Requirement: VIVADO_HW_SERVER_URL exposed to in-container TCL

The container SHALL receive `VIVADO_HW_SERVER_URL` in its environment whenever JTAG
host-broker mode is active. The intended Vivado TCL usage pattern SHALL be the
documented one-liner:

```
connect_hw_server -url $::env(VIVADO_HW_SERVER_URL)
```

Vivado SHALL NOT auto-invoke this connect at startup. The operator (or a project's TCL
hook) decides when to attach to hardware.

#### Scenario: TCL connect uses env var
- **WHEN** a Vivado TCL session inside a `--jtag-host` container executes
  `connect_hw_server -url $::env(VIVADO_HW_SERVER_URL)`
- **THEN** the connection attempt targets the URL the launcher resolved, with no further
  configuration required from the operator.

### Requirement: In-container USB JTAG plumbing absent

The launcher SHALL NOT include any of the previously planned in-container USB JTAG
mechanisms. Specifically:

- No `-v /dev/bus/usb:/dev/bus/usb` bind mount under any flag.
- No `--device-cgroup-rule 'c 189:* rmw'`.
- No `-v /run/udev:/run/udev:ro` bind mount.
- No host udev rule template files shipped under `etc/udev/` in the repository.

The repository SHALL NOT carry templates for Xilinx host-side udev rules; operators
wishing to use the rejected in-container path are expected to fork.

#### Scenario: docker run command excludes USB plumbing
- **WHEN** the launcher's full docker run command line is inspected with `--gui` or
  `--jtag-host`
- **THEN** none of the strings `--device-cgroup-rule`, `/dev/bus/usb`, or `/run/udev`
  appears in the args.

#### Scenario: Repository has no host udev rule templates
- **WHEN** the repository tree is inspected at `etc/udev/`
- **THEN** the directory either does not exist or is empty.

---

## Cross-capability constraints

- **Mode exclusivity:** GUI mode and headless daemon mode share no runtime state; mode
  is fixed at container start via `VIVADO_MODE` (§4).
- **UID alignment applies in both modes** (§4, §5): `HOST_UID`/`HOST_GID` are required
  in the entrypoint root branch regardless of mode.
- **libudev stub applies in both modes** (§6): activated via `LD_LIBRARY_PATH` on the
  Vivado launch chain only; never system-wide.
- **`--gui` implies `--jtag-host`** (§7): every GUI container has
  `VIVADO_HW_SERVER_URL` and the `host.docker.internal` host-gateway entry.
- **Lifecycle invariant** (§2, §3, §4): in every mode, tini is PID 1, the application
  (daemon or GUI Vivado) is PID 2 after the gosu pivot, and the container exits with the
  application's exit status.
- **Version note:** the session-container capabilities (§2–§7) are specified against the
  `vivado-vitis-24.2` base image; §1 specifies the 2025.2 base image
  (`Dockerfile-vivado-25.2`) built with the same authenticated batch-install pattern.
