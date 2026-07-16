# docs-structure — Delta Specification

## MODIFIED Requirements

### Requirement: README is the single source of usage instructions

`README.md` SHALL be the authoritative document for prerequisites, build,
run, environment variables, daVit session/CLI usage (including the
sidecar compose pattern), and troubleshooting. It SHALL NOT document
Apple Silicon/Rosetta support or a `ROSETTA` environment variable.
It SHALL include a "Repository layout" section describing the folder
structure and the purpose of each top-level folder.

#### Scenario: Env vars documented in README
- **WHEN** a user needs the list of supported environment variables
  (`VIVADO_VERSION`, `SRC_DIR`, `WORK_DIR`, `USB_DEVICE_DIR`)
- **THEN** they are fully documented in README.md and `ROSETTA` is not
  among them

#### Scenario: daVit usage documented
- **WHEN** a user needs to start a persistent session, run TCL, or wire
  the image as a compose sidecar
- **THEN** README.md documents `dv` verbs, the `.dv/` session root, and
  a compose example with `service_healthy` gating

#### Scenario: Repository layout section
- **WHEN** a reader opens README.md
- **THEN** a "Repository layout" section lists `scripts/`, `config/`,
  `docker/`, `davit/`, and `docs/` with one-line descriptions
