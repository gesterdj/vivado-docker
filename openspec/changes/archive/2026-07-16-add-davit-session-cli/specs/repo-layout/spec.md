# repo-layout — Delta Specification

## MODIFIED Requirements

### Requirement: Canonical folder structure

The repository SHALL organize files by kind into dedicated top-level
folders: `scripts/` for executable helper scripts, `config/` for installer
configuration files, `docker/` for container build sources, `davit/` for
the daVit Rust crate (session daemon + `dv` CLI), and `docs/`
for supplementary documentation. Within `docker/`, each image SHALL have
its own subfolder containing its Dockerfile: `docker/base/` for the
Vivado/Vitis base image and `docker/tools/` for the tools overlay image;
shared build sources (e.g., `udev_stub.c`) SHALL live directly under
`docker/`. `Makefile`, `README.md`, `AGENTS.md`, and `LICENSE` SHALL
remain at the repository root.

#### Scenario: Run script location
- **WHEN** a user looks for the Vivado run script
- **THEN** it is found at `scripts/run.vivado.sh` and is executable

#### Scenario: Launcher location
- **WHEN** a user looks for the daVit host launcher
- **THEN** it is found at `scripts/dv` and is executable

#### Scenario: Crate location
- **WHEN** a contributor looks for the daVit source
- **THEN** the Rust crate rooted at `davit/Cargo.toml` builds the `dv`
  binary, and no daVit source lives outside `davit/`

#### Scenario: Installer configs location
- **WHEN** the Docker base image is built
- **THEN** installer configuration is read from
  `config/install_config.txt`

#### Scenario: Per-image Dockerfile folders
- **WHEN** a contributor looks for a Dockerfile
- **THEN** the base image Dockerfile is at `docker/base/Dockerfile` and
  the tools overlay at `docker/tools/Dockerfile`, with no Dockerfile
  directly under `docker/`

#### Scenario: Supplementary docs location
- **WHEN** a contributor looks for the project spec document
- **THEN** `fpgatools-docker-spec.md` is found under `docs/`
