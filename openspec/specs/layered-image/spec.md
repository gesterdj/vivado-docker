# layered-image Specification

## Purpose
TBD - created by archiving change multi-stage-image-layers. Update Purpose after archive.
## Requirements
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
The overlay SHALL contain all environment customization currently in the
single-stage image: the libudev Rosetta stub compiled to
`/opt/udev_stub.so` (path unchanged), the developer package set,
`VOLUME /src`, `VOLUME /work`, and `WORKDIR /work`. New customization
SHALL be added to overlays, not to the base image.

#### Scenario: Rosetta stub present at known path
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

