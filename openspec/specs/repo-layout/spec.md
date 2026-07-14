# repo-layout Specification

## Purpose
Define the canonical repository folder layout and required build/documentation
references for Vivado Docker tooling.

## Requirements

### Requirement: Canonical folder structure
The repository SHALL organize files by kind into dedicated top-level
folders: `scripts/` for executable helper scripts, `config/` for installer
configuration files, `docker/` for Dockerfile and container build sources,
and `docs/` for supplementary documentation. `Makefile`, `README.md`,
`AGENTS.md`, and `LICENSE` SHALL remain at the repository root.

#### Scenario: Run script location
- **WHEN** a user looks for the Vivado run script
- **THEN** it is found at `scripts/run.vivado.sh` and is executable

#### Scenario: Installer configs location
- **WHEN** the Docker image is built
- **THEN** installer configuration is read from `config/install_config.txt`
- **AND** `config/xsetup_config_25.txt` resides in the same folder

#### Scenario: Supplementary docs location
- **WHEN** a contributor looks for the project spec document
- **THEN** `fpgatools-docker-spec.md` is found under `docs/`

### Requirement: Build tooling references new paths
The `Makefile`, `docker/Dockerfile`, and `.dockerignore` SHALL reference
the relocated files so that `make build` works unchanged from the root.

#### Scenario: Make build after restructure
- **WHEN** `make HOST_TOOL_ARCHIVE_NAME=<archive> build` is invoked
- **THEN** the build proceeds using `config/install_config.txt` without
  path errors

#### Scenario: No dangling references
- **WHEN** the repository is searched for `run.sh`, root-level
  `install_config.txt`, or root-level `run.vivado.sh` references
- **THEN** no build file or documentation references the old paths

### Requirement: Placeholder script removal
The empty placeholder `run.sh` SHALL be removed.

#### Scenario: run.sh removed
- **WHEN** the repository root is listed after the change
- **THEN** `run.sh` does not exist and nothing references it
