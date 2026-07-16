# Latest: 2025.2

# vivado-docker

## Table of Contents
* [Summary](#summary)
* [Why?](#why)
* [Repository layout](#repository-layout)
* [Prerequisites](#prerequisites)
* [Limitations](#limitations)
* [Maintenance](#maintenance)
* [daVit: Persistent Session CLI](#davit-persistent-session-cli)
* [Troubleshooting/FAQ](#troubleshootingfaq)
* [Contribution](#contribution)
* [Prior art](#prior-art)

## Summary

This repository provides a [Docker](https://docker.io) setup for AMD's [Vivado][viv]
FPGA development tools, specifically version 2025.2.

[viv]: https://en.wikipedia.org/wiki/Vivado

**Important:** This repository does not contain any Vivado software. Instead, it
offers a recipe to build your own Docker container using a Vivado installer that
you download from AMD. Due to its size and licensing restrictions, the built
Docker image is not available for download from Docker Hub or other public
registries.

The build produces a base Docker image with a pre-configured installation
of AMD's (formerly Xilinx) Vivado/Vitis tools, plus a thin tools overlay
image for fast environment customization. The initial base build is a
time-consuming process (multiple hours); overlay rebuilds take minutes.

By default, the build installs a free-to-use selection of devices and
tools from the Vitis Unified Software Platform, as configured in
`config/install_config.txt`.

## Why?

This project aims to provide a repeatable, hermetic, and self-maintaining
development environment using Docker containers. This ensures a consistent Vivado
setup across different machines. If this is not a concern for you, a standard
Vivado installation may be sufficient.

[bzl]: https://www.hdlfactory.com/tags/bazel/

## Repository layout

| Folder     | Contents                                                    |
|------------|-------------------------------------------------------------|
| `scripts/` | Helper scripts (`dv`, `run.vivado.sh`, `build.*.sh`, `gen_auth_token.sh`) |
| `config/`  | Installer configuration (`install_config.txt`)             |
| `docker/`  | Container build sources; `base/` and `tools/` Dockerfiles   |
| `davit/`   | daVit session daemon + `dv` CLI (Rust crate, static binary) |
| `docs/`    | Supplementary documentation                                 |

`Makefile`, `README.md`, `AGENTS.md`, and `LICENSE` live at the root.

## Prerequisites

*   Git
*   Docker (with BuildKit enabled — required for bind mounts and secrets)
*   The AMD FPGAs & Adaptive SoCs Web ("slim") Installer and a valid AMD
    account (for which you hold a valid license).

## Limitations

This solution for dockerizing Vivado has the following known limitations:

*   **Supported Edition:** This project installs the Vitis Unified
    Software Platform (which includes Vivado) via the AMD web installer,
    selecting only free-to-use tools and devices in
    `config/install_config.txt`. Paid editions and their licensing
    mechanisms are not supported.
*   **Installer Availability:** You must download the AMD web installer
    yourself directly from AMD and log in with your own AMD account. This
    repository cannot and will not provide the installer due to licensing
    and distribution restrictions.
*   **Testing Constraints:** Thoroughly testing all possible configurations and
    Vivado versions is challenging due to the multi-hour installer downloads
    and build times.
*   **Linux x86\_64 hosts only.** The image and helper scripts target
    native `linux/amd64` Docker hosts.

## Maintenance

### Preparing for the Build

The image is built in two stages: a **base image**
(`xilinx-vivado-base:<version>`, contains the Vivado/Vitis installation,
rebuilt rarely) and a **tools image** (`xilinx-vivado:<version>`, thin
overlay with dev packages and stubs, rebuilds in minutes). Tool packages
are downloaded by the AMD web installer during the base build — no ~50GB
archive is copied around.

1.  **Download the Slim (Web) Installer:** Obtain the AMD FPGAs &
    Adaptive SoCs "Web Installer" `.bin` from AMD and place it in the
    `installer/` directory. You are responsible for complying with all
    software licensing terms.
2.  **Generate an auth token:**

    ```bash
    make auth-token   # auto-detects the *.bin in installer/
    ```

    This runs AMD's own `AuthTokenGen` (interactive login) and writes
    `~/.Xilinx/wi_authentication_key`. The token is passed to the build
    as a BuildKit secret — your credentials never enter the build, and
    the token is never stored in an image layer. Tokens expire; re-run
    this step if a build fails to authenticate. (This deviates from the
    spec's build-arg credential approach on purpose: build args leak
    into `docker history`.)
3.  **Review `config/install_config.txt`:** edit `Modules=` to select
    the devices/tools to install.

### Building the Container

Navigate to the repository's root directory and run:

```bash
make build-base   # slow: web install of Vivado/Vitis (rare)
make build        # fast: tools overlay (after any customization change)
```

`make build` triggers the base build automatically when the base image
or its stamp is missing (including after `docker rmi`). Customize the
environment in `docker/tools/Dockerfile` — overlay rebuilds never re-run
the installer.

The actual `docker build` invocations live in `scripts/build.base.sh`
and `scripts/build.tools.sh`; `make` adds stamp-based caching on top.
Both scripts honor `VIVADO_VERSION`, and the base script additionally
honors `INSTALLER` and `AUTH_TOKEN_FILE`. Call them directly to force
a rebuild without touching the stamps.

The base build is lengthy. See the FAQ section for more details on build
times and optimizations.

### Saving the Image

After a successful build, you can save the Docker image to a `.tar` archive:

```bash
make save
```

This archive (e.g., `xilinx-vivado.2025.2.docker.tgz`) can be transferred
to other machines. The image is too large for Docker Hub and is not hosted
there.

### Loading the Image

To load the image from an archive:

```bash
docker load -i xilinx-vivado.2025.2.docker.tgz
```

Note: Loading very large Docker images can sometimes be unreliable. See the FAQ
section for more details.

### Running Vivado from the Image

Once the image is loaded into Docker, start Vivado using:

```bash
make run
```

By default this starts the Vivado GUI (requires X11 passthrough on Linux).
Or use `scripts/run.vivado.sh` directly with environment overrides:

```bash
# Interactive TCL console
VIVADO_CMD="vivado -mode tcl" ./scripts/run.vivado.sh

# Batch synthesis
SRC_DIR=/path/to/fpga/project WORK_DIR=/path/to/output \
  VIVADO_CMD="vivado -mode batch -source /src/build.tcl" \
  ./scripts/run.vivado.sh
```

**`run.vivado.sh` environment variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `VIVADO_VERSION` | `2025.2` | Vivado version to use |
| `SRC_DIR` | current directory | Host directory mounted at `/src` |
| `WORK_DIR` | current directory | Host directory mounted at `/work` |
| `VIVADO_CMD` | `vivado` (GUI) | Command to run inside container |
| `USB_DEVICE_DIR` | None | Set to Host USB dir in order to enable USB |

## daVit: Persistent Session CLI

`run.vivado.sh` starts a fresh Vivado for every invocation. **daVit**
(binary: `dv`) instead keeps one warm Vivado TCL session alive in a
container and lets you fire commands at it — from the host, from
scripts, or from sibling containers sharing the workspace. Vivado's
multi-minute startup cost is paid once per session, not once per
command.

### Quick start

```bash
cd /path/to/project           # contains myproj.xpr
/path/to/vivado-docker/scripts/dv start --project myproj.xpr
dv() { ./.dv/bin/dv "$@"; }   # or keep using scripts/dv

dv exec 'get_projects'
dv exec -- report_utilization -file util.rpt
dv show status
dv stop
```

`start` creates the container (`docker run --init -u UID:GID` with the
project directory mounted at `/workspace`) and waits for readiness.
Everything else talks to the running session through the session root.

### Session root: `.dv/`

The daemon creates `<workspace>/.dv/` containing the control socket
(`control.sock`, owner-only), session artifacts (`metadata.json`,
`result.json`, `health.json`), timestamped raw session logs, and a
self-published copy of the CLI at `.dv/bin/dv`. Any process that can
see the workspace — including sibling containers with zero
preinstalled dependencies — gets the full client by running
`.dv/bin/dv`. Add `.dv/` to your project's `.gitignore`; it is
per-session machine state.

### Verbs

| Verb | Purpose |
|------|---------|
| `dv start [headless\|gui]` | Create the session container (host only) |
| `dv exec [--timeout S] [--file F] [--] TCL...` | Run one TCL operation |
| `dv show status\|result\|metadata\|health [--json]` | Session state from artifacts |
| `dv logs [--tail N] [--follow]` | Raw session log (file read only) |
| `dv diagnose last\|health\|inspect\|ps\|wchan\|...` | Read-only probes, never touches the session |
| `dv run xsct\|xsdb\|bootgen\|dtc ARGS...` | Managed companion-tool operation |
| `dv stop [--force]` | Graceful stop; `--force` needs the host launcher |

Exit codes: `0` success, `1` Vivado error/busy/crash, `2` usage or no
result, `3` client wait timed out (the command keeps running — retrieve
it later with `dv show result`). One command runs at a time; concurrent
`exec` calls are rejected immediately with `busy`, never queued.

INFO and WARNING lines are filtered from `exec` output by default
(ERROR/CRITICAL WARNING always surface; everything lands unfiltered in
the raw log). To retain specific warnings, list message IDs in
`<workspace>/elfws.yaml`:

```yaml
# warning IDs to suppress; all others are retained
- Synth 8-7080
- Vivado 12-1
```

### Sidecar usage (compose)

The session container works as a sidecar: siblings share the workspace
volume and gate on session readiness via the image `HEALTHCHECK`:

```yaml
services:
  vivado:
    image: xilinx-vivado:2025.2
    init: true
    user: "1000:1000"
    command: ["session", "--project", "myproj.xpr"]
    volumes: [ "./:/workspace" ]

  builder:
    image: your-build-image
    depends_on:
      vivado:
        condition: service_healthy
    volumes: [ "./:/workspace" ]
    command: ["/workspace/.dv/bin/dv", "exec", "--file", "build.tcl"]
```

Sibling containers can `exec`, `show`, `logs`, `diagnose`, `run`, and
gracefully `stop` the session; container lifecycle (`start`,
`stop --force`) belongs to the orchestrator.

### GUI mode

`dv start gui` runs `vivado -mode gui` as the container's foreground
application — no daemon, socket, or session artifacts. Display
plumbing is auto-detected (`wsl`, `wayland`, `x11`) and can be forced
with `--gui-profile`. Hardware access goes through a host `hw_server`:
`--jtag-host[=HOST:PORT]` (default `host.docker.internal:3121`, implied
in GUI mode) exports `VIVADO_HW_SERVER_URL`; in Vivado connect with
`open_hw_target -host ...`.

### Launcher environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VIVADO_VERSION` | `2025.2` | Image tag to run |
| `DV_IMAGE` | `xilinx-vivado:$VIVADO_VERSION` | Full image override |
| `DV_WORKSPACE` | current directory | Workspace directory |
| `DV_JTAG_HOST` | `host.docker.internal:3121` | Default `--jtag-host` value |
| `DV_STARTUP_TIMEOUT` | `600` | Readiness wait in seconds |

## Troubleshooting/FAQ

**Q: Why does the Docker build take so long (several hours)?**

A: The Vivado installation is very large, and the process itself is complex.
The base build downloads the selected tool packages via the AMD web
installer, runs the installer, and finally exports the numerous layers of
the resulting Docker image. The slim installer `.bin` is bind-mounted and
self-extracted inside the install layer (never persisted in an image
layer), and once the base image exists, customization rebuilds
(`make build`) take only minutes. The initial base build will still be
lengthy.

**Q: The Docker image is huge (tens to hundreds of GB). Is this normal?**

A: Yes, this is unfortunately normal. Vivado is a comprehensive tool suite
with a very large number of files and libraries. The final size depends on
the devices and tools selected in `config/install_config.txt` — keep the
module selection minimal to keep the image manageable.

**Q: My Docker build fails with errors related to X11 or display servers. What can I do?**

A: This script builds Vivado in a headless environment (without a graphical
display). Some Vivado installation options or components might require an X11
display server during the installation itself. This script does not support such
options. Ensure your `config/install_config.txt` only selects components
compatible with a headless installation.

**Q: How do I choose which Vivado components are installed?**

A: You can customize the installation by editing
`config/install_config.txt` *before* starting the base build. In the
`Modules=` section, enable or disable devices/components by changing their
value from `:0` (disabled) to `:1` (enabled). A template can be generated
with `` `xsetup -b ConfigGen` `` from the slim installer.

**Q: Vivado crashes with `realloc(): invalid pointer` at startup.**

A: Vivado's license manager and WebTalk call
`udev_enumerate_scan_devices()`, which can misbehave inside containers
that have no udev daemon or device database. The image includes a
libudev stub at `/opt/udev_stub.so` that provides no-op
implementations. `run.vivado.sh` and the daVit entrypoint apply it
automatically, scoped to the Vivado process tree. If running Vivado
manually inside the container, add: `export
LD_PRELOAD=/opt/udev_stub.so` before sourcing `settings64.sh`.

**Q: `launch_runs` crashes but `synth_design` works. Why?**

A: `launch_runs` spawns child processes which each independently load
`libudev`, and the preload may not propagate correctly to all children.
Use in-process TCL commands instead: `synth_design`, `opt_design`,
`place_design`, `route_design`, `write_bitstream`.

**Q: `` `docker load -i xilinx-vivado.2025.2.docker.tgz` `` fails or takes many attempts. Any advice?**

A: Loading extremely large Docker image archives can be unreliable with some
versions or configurations of Docker. Ensure you have sufficient disk space in
your Docker daemon's storage location (check Docker settings). Trying the command
again sometimes helps. If persistent issues occur, consider checking Docker
daemon logs for more specific errors or consulting Docker community forums for
advice on handling large images.

**Q: USB devices are not showing up in the Hardware Manager. Why?**

A: You need to set the environment variable USB_DEVICE_DIR to the host's 
path to USB devices. On Ubuntu, this path is by default `/dev/bus/usb`

**Q: USB devices are still not showing up. Why?**

A: The image does not install USB cable drivers — the host OS needs the
drivers installed. These can be extracted from the Docker image using the
following commands (note that if you use a different version than 2025.2,
change the numbers):
```
docker create --name tmp xilinx-vivado:2025.2
docker cp tmp:/opt/Xilinx/2025.2/Vivado/data/xicom/cable_drivers ./cable_drivers
```
This will extract the cable drivers for both Windows and Linux in your
current directory. Install the appropriate drivers on the host OS. 
You may need to unplug and plug the USB device before the drivers work.

## Contribution

Contributions are welcome! Please feel free to submit pull requests or open
issues.

## Prior art

This repo was not built in a vacuum. I consulted a number of resources out
there on the internet.

* [Dockerizing Xilinx tools.][1] discussion on Reddit, which bootstrapped this
  work.
* [Xilinx tools docker][8]: the freshest piece of instruction that I could find.
* [Xilinx Vivado with Docker and Jenkins][2]. Does what it says on the tin.
* [Xilinx Vivado/Vivado HLS][3] from CERN.
* [Xilinx guides about Docker][4], which I'm not sure helped at all.
* [AMD guides about Vivado on Kubernetes et al.][5].
* [Install Xilinx Vivado using Docker][6] [link broken?], another blog recount of the process.
* [Run GUI applications in Docker or podman containers.][7]
* [Dockerized Vivado ML Enterprise by esnet][esnet].

[1]: https://www.reddit.com/r/FPGA/comments/bk8b3n/dockerizing_xilinx_tools/
[2]: https://www.starwaredesign.com/index.php/blog/64-fpga-meets-devops-xilinx-vivado-and-jenkins-with-docker
[3]: https://github.com/aperloff/vivado-docker
[4]: https://xilinx.github.io/Xilinx_Container_Runtime/docker.html
[5]: https://docs.xilinx.com/r/en-US/Xilinx_Kubernetes_Device_Plugin/1.-Install-Docker
[6]: https://blog.p4ck3t0.de/post/xilinx_docker/
[esnet]: https://github.com/esnet/xilinx-tools-docker
[7]: https://github.com/mviereck/x11docker
[8]: https://github.com/esnet/xilinx-tools-docker/tree/main
