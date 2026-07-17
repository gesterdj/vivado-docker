# Delta: cli-frontend (add-parallel-gui-launch)

## MODIFIED Requirements

### Requirement: Launcher owns runtime verbs only

The repo SHALL ship a thin host launcher `scripts/dv` whose sole
runtime responsibilities are: `start` (validate inputs, create the
container via the OCI runtime with `-u "$(id -u):$(id -g)"`, workspace
bind-mounted at `/workspace`, `--init`, then wait for readiness) and
the `--force` escalation of `stop` (runtime SIGTERM/kill when the
daemon is unreachable). Every other invocation SHALL be delegated
verbatim to `<workspace>/.dv/bin/dv`. When no session root exists, the
launcher SHALL print a hint to run `dv start` and exit `2`.

Container names SHALL be per-mode: headless sessions use
`davit-<wshash>` and GUI sessions use `davit-<wshash>-gui`, where
`<wshash>` is the deterministic workspace hash. GUI containers SHALL
carry the label `davit.session=gui` in addition to the workspace
ownership label. The `start` guard SHALL inspect only the container
name belonging to the requested mode. `start` SHALL NOT replace or
adopt an unrelated container; repeated `start headless` of the same
healthy session SHALL be idempotent, and `start gui` while a GUI
container for this workspace is running SHALL be refused with a
non-zero exit. Stale stopped containers owned by this workspace SHALL
be removed before creating a replacement, for either mode.

`stop --force` SHALL stop and remove every workspace-owned container
among `davit-<wshash>` and `davit-<wshash>-gui`, and SHALL error only
when neither exists.

#### Scenario: Delegation to published binary
- **WHEN** the host user runs `scripts/dv exec 'get_projects'` in a
  workspace with a live session
- **THEN** the launcher execs `.dv/bin/dv exec get_projects` and the
  exit status is the published binary's

#### Scenario: Duplicate start is idempotent
- **WHEN** `dv start headless` runs while the managed session container
  is already healthy
- **THEN** the command reports current status and exits `0` without
  creating a second container

#### Scenario: Duplicate GUI start is refused
- **WHEN** `dv start gui` runs while `davit-<wshash>-gui` for this
  workspace is already running
- **THEN** the launcher prints a "gui already open" error and exits
  non-zero without creating a container

#### Scenario: Force stop sweeps both containers
- **WHEN** `dv stop --force` runs while both a headless session and a
  GUI container exist for this workspace
- **THEN** both containers are stopped and removed, and the command
  reports each removal

## ADDED Requirements

### Requirement: GUI and headless sessions coexist per workspace

`dv start gui` SHALL succeed while a headless session container is
running in the same workspace, and `dv start headless` SHALL succeed
while a GUI container is running. The GUI container SHALL remain
session-free: no daemon, no control socket, and no writes under
`.dv/`; all delegated verbs (`exec`, `show`, `logs`, `diagnose`,
`run`) SHALL continue to address the headless session exclusively.
Concurrent access to the same project from both containers is the
user's responsibility; the launcher SHALL NOT add project-level
locking.

#### Scenario: GUI opens alongside a running batch session
- **WHEN** a headless session is running and the user runs
  `dv start gui`
- **THEN** a GUI container named `davit-<wshash>-gui` starts, and
  `dv exec` against the headless session continues to work while the
  GUI is open

#### Scenario: GUI leaves no session artifacts
- **WHEN** a GUI container runs and exits
- **THEN** the contents of `<workspace>/.dv/` are unchanged by the GUI
  container
