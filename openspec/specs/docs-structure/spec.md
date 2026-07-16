# docs-structure Specification

## Purpose
Define where user-facing usage instructions and agent-facing repository
guidance live to avoid duplicated documentation.

## Requirements

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

### Requirement: AGENTS.md contains only agent guidance without duplication
`AGENTS.md` SHALL contain only agent-facing conventions (commit message
format, markdown formatting rules, project constraints) and SHALL link to
README.md sections for build/run/environment instructions instead of
duplicating them.

#### Scenario: No duplicated instructions
- **WHEN** AGENTS.md is compared with README.md
- **THEN** build steps, run commands, and environment variable tables
  appear only in README.md, with AGENTS.md linking to the relevant
  sections

#### Scenario: Conventions retained
- **WHEN** an agent reads AGENTS.md
- **THEN** it finds the Conventional Commits rule, the 80-column markdown
  rule, and the project constraints (ML Standard only, headless build, no
  public image hosting)
