## ADDED Requirements

### Requirement: README is the single source of usage instructions
`README.md` SHALL be the authoritative document for prerequisites, build,
run, environment variables, Apple Silicon support, and troubleshooting.
It SHALL include a "Repository layout" section describing the folder
structure and the purpose of each top-level folder.

#### Scenario: Env vars documented in README
- **WHEN** a user needs the list of supported environment variables
  (`VIVADO_VERSION`, `SRC_DIR`, `WORK_DIR`, `USB_DEVICE_DIR`, `ROSETTA`)
- **THEN** they are fully documented in README.md

#### Scenario: Repository layout section
- **WHEN** a reader opens README.md
- **THEN** a "Repository layout" section lists `scripts/`, `config/`,
  `docker/`, and `docs/` with one-line descriptions

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
