# Latest: 2025.2

# vivado-docker

## Table of Contents
* [Summary](#summary)
* [Why?](#why)
* [Repository layout](#repository-layout)
* [Prerequisites](#prerequisites)
* [Limitations](#limitations)
* [Apple Silicon / Rosetta Support](#apple-silicon--rosetta-support)
* [Maintenance](#maintenance)
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
| `scripts/` | Helper scripts (`run.vivado.sh`, `gen_auth_token.sh`)       |
| `config/`  | Installer configuration (`xsetup_config_25.txt`)            |
| `docker/`  | Container build sources; `base/` and `tools/` Dockerfiles   |
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
    Vivado versions is challenging due to the dependency on specific, large
    installer archives from AMD and the lengthy build times.

## Apple Silicon / Rosetta Support

This setup works on Apple Silicon Macs (M1/M2/M3/M4) via Rosetta x86\_64
emulation in Docker (OrbStack or Docker Desktop). Key adaptations:

*   **`--platform linux/amd64`** is added to all Docker commands (build & run)
*   **libudev stub**: Vivado's license manager and WebTalk telemetry call
    `udev_enumerate_scan_devices()` which crashes under Rosetta with
    `realloc(): invalid pointer`. A stub shared library at `/opt/udev_stub.so`
    is built into the image and loaded via `LD_PRELOAD` automatically when
    running on ARM64 hosts.
*   **`launch_runs` may crash** under Rosetta because it spawns child processes
    that also trigger the libudev crash. For synthesis, prefer in-process
    commands (`synth_design`, `place_design`, `route_design`) over `launch_runs`
    in your TCL scripts.

On native x86\_64 hosts, the Rosetta workarounds are harmless but unnecessary.

## Maintenance

### Preparing for the Build

The image is built in two stages: a **base image**
(`xilinx-vivado-base:<version>`, contains the Vivado/Vitis installation,
rebuilt rarely) and a **tools image** (`xilinx-vivado:<version>`, thin
overlay with dev packages and stubs, rebuilds in minutes). Tool packages
are downloaded by the AMD web installer during the base build — no ~50GB
archive is copied around.

1.  **Download the Slim (Web) Installer:** Obtain the AMD FPGAs &
    Adaptive SoCs "Web Installer" `.bin` from AMD. You are responsible
    for complying with all software licensing terms.
2.  **Unpack the installer** into `./Xilinx/<version>/` at the repo root
    (e.g., `./Xilinx/2025.2/xsetup` must exist). This tree is
    bind-mounted into the build, never copied into a layer.
3.  **Generate an auth token:**

    ```bash
    make auth-token INSTALLER=./FPGAs_AdaptiveSoCs_Unified_..._Web.bin
    ```

    This runs AMD's own `AuthTokenGen` (interactive login) and writes
    `~/.Xilinx/wi_authentication_key`. The token is passed to the build
    as a BuildKit secret — your credentials never enter the build, and
    the token is never stored in an image layer. Tokens expire; re-run
    this step if a build fails to authenticate. (This deviates from the
    spec's build-arg credential approach on purpose: build args leak
    into `docker history`.)
4.  **Review `config/install_config.txt`:** edit `Modules=` to select
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

The base build is lengthy. See the FAQ section for more details on build
times and optimizations.

### Saving the Image

After a successful build, you can save the Docker image to a `.tar` archive:

```bash
make save
```

This archive (e.g., `xilinx-vivado.docker.tgz`) can be transferred to other
machines. The image is too large for Docker Hub and is not hosted there.

### Loading the Image

To load the image from an archive:

```bash
docker load -i xilinx-vivado.docker.tgz
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
| `ROSETTA` | auto-detect | Set to `1` to force libudev stub |
| `USB_DEVICE_DIR` | None | Set to Host USB dir in order to enable USB |

## Troubleshooting/FAQ

**Q: Why does the Docker build take so long (several hours)?**

A: The Vivado installation is very large, and the process itself is complex.
The base build downloads the selected tool packages via the AMD web
installer, runs the installer, and finally exports the numerous layers of
the resulting Docker image. The slim installer tree is bind-mounted (never
copied into the build context), and once the base image exists,
customization rebuilds (`make build`) take only minutes. The initial base
build will still be lengthy.

**Q: The Docker image is over 200GB. Is this normal?**

A: Yes, this is unfortunately normal. Vivado is a comprehensive tool suite, and
a full installation contains a very large number of files and libraries, leading
to a massive Docker image.

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

**Q: Vivado crashes with `realloc(): invalid pointer` on Apple Silicon.**

A: This is a known issue with Rosetta x86\_64 emulation. Vivado's license
manager calls `udev_enumerate_scan_devices()` which triggers a crash in glibc's
allocator under Rosetta. The image includes a libudev stub at
`/opt/udev_stub.so` that provides no-op implementations. `run.vivado.sh`
applies this automatically on ARM64 hosts. If running Vivado manually inside
the container, add: `export LD_PRELOAD=/opt/udev_stub.so` before sourcing
`settings64.sh`.

**Q: `launch_runs` crashes but `synth_design` works. Why?**

A: `launch_runs` spawns child processes which each independently load
`libudev`. Under Rosetta, these children crash even with `LD_PRELOAD` set
(the preload may not propagate correctly to all children). Use in-process TCL
commands instead: `synth_design`, `opt_design`, `place_design`, `route_design`,
`write_bitstream`.

**Q: `` `docker load -i xilinx-vivado.docker.tgz` `` fails or takes many attempts. Any advice?**

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
