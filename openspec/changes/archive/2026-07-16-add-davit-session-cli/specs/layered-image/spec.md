# layered-image — Delta Specification

## ADDED Requirements

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

## MODIFIED Requirements

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
