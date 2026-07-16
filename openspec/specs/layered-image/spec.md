# layered-image Specification

## Purpose
Define the two-image layered build (base + tools overlay) and the daVit
session binary built inside the overlay.

## Requirements

### Requirement: davit binary built in a cached builder stage

`docker/tools/Dockerfile` SHALL build the `davit` crate in a dedicated
Rust builder stage targeting `x86_64-unknown-linux-musl`, and the final
stage SHALL copy only the resulting static binary (installed as
`/opt/davit/dv`). The Rust toolchain SHALL NOT be present in the final
image. The image SHALL define an entrypoint that dispatches session
modes (`session`, `gui`) to the binary and a `HEALTHCHECK` based on
session readiness. Image count SHALL remain two (base + tools).

#### Scenario: Static binary, no toolchain
- **WHEN** the tools image is inspected
- **THEN** `/opt/davit/dv` is a statically linked executable and no
  `cargo`/`rustc` is present in the final image

#### Scenario: Cached rebuild stays fast
- **WHEN** only `davit/` source changes and `make build` is rerun
- **THEN** only the builder and final overlay layers rebuild; the base
  image and apt layers are untouched

### Requirement: Overlay image extends the base by tag
The tools overlay (`docker/tools/Dockerfile`) SHALL start `FROM
xilinx-vivado-base:<version>` and SHALL produce the image
`xilinx-vivado:<version>` consumed by `scripts/run.vivado.sh`. The base
image SHALL NOT be rebuilt by overlay builds.

#### Scenario: Overlay rebuild leaves base untouched
- **WHEN** `docker/tools/Dockerfile` is modified and `make build` is run
  with an existing `xilinx-vivado-base:<version>`
- **THEN** only overlay layers are rebuilt — no installer download or
  xsetup execution occurs

#### Scenario: Run script keeps working
- **WHEN** `scripts/run.vivado.sh` starts a container after an overlay
  build
- **THEN** it resolves image `xilinx-vivado:<version>` unchanged

### Requirement: Overlay owns environment customization

The overlay SHALL contain all environment customization: the universal
libudev stub compiled to `/opt/udev_stub.so` (path unchanged; a
Vivado-in-Docker mitigation, not Apple-specific), the developer package
set, the `davit` session binary and entrypoint, `VOLUME /src`,
`VOLUME /work`, and `WORKDIR /work`. New customization SHALL be added
to overlays, not to the base image.

#### Scenario: udev stub present at known path
- **WHEN** the overlay image is inspected
- **THEN** `/opt/udev_stub.so` exists and is loadable via `LD_PRELOAD`

#### Scenario: Volumes and workdir preserved
- **WHEN** a container starts from `xilinx-vivado:<version>`
- **THEN** `/src` and `/work` are declared volumes and the working
  directory is `/work`

### Requirement: Two-target Makefile build pipeline
The Makefile SHALL provide `auth-token` (runs
`scripts/gen_auth_token.sh`), `build-base` (builds
`xilinx-vivado-base:<version>`, failing with a hint to run
`make auth-token` when the token file is absent), and `build` (builds
the overlay, depending on the base stamp). Stamp-based skipping SHALL
be backed by `docker image inspect` so that a deleted image triggers a
rebuild despite a current stamp. `HOST_TOOL_ARCHIVE_NAME`
SHALL no longer be required.

#### Scenario: Guided flow
- **WHEN** the operator runs `make build-base` without a token file
- **THEN** the build fails fast with a message pointing to
  `make auth-token`

#### Scenario: build depends on base
- **WHEN** `make build` is run and no base stamp/image exists
- **THEN** the base build is triggered first, then the overlay

#### Scenario: Stale stamp after image removal
- **WHEN** `base.stamp` is current but `docker rmi
  xilinx-vivado-base:<version>` has removed the image
- **THEN** `make build-base` detects the missing image via
  `docker image inspect` and rebuilds instead of skipping
