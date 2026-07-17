# Proposal: add-parallel-gui-launch

## Why

`dv start gui` cannot run while a headless session is active in the
same workspace: both modes claim the single deterministic container
name `davit-<wshash>`, and the start guard refuses before Docker is
even consulted. The intended workflow is batch-first — a persistent
headless session driven via `dv exec` — with a human occasionally
opening a GUI on the same workspace to investigate current status
(reports, placement, schematics). Today that requires bypassing `dv`
with a raw `docker run`.

## What Changes

- GUI containers get their own name: `davit-<wshash>-gui`, with label
  `davit.session=gui`. Headless container naming is unchanged (zero
  migration).
- The start guard becomes mode-aware: `dv start gui` checks only the
  `-gui` container; `dv start headless` checks only its own. A GUI may
  be launched while a headless session is running, and vice versa.
- A second `dv start gui` in the same workspace is refused with a
  clear "gui already open" message.
- `dv stop --force` cleans up both the headless and the GUI container
  (whichever exist). GUI containers normally vanish on window close
  via `--rm`.
- GUI mode remains socket-free: no daemon, no `.dv/` writes, no
  session artifacts. It stays invisible to all delegated verbs
  (`exec`, `show`, `logs`, ...), which continue to address the
  headless session only.
- Concurrent access to the same `.xpr` from both sessions is the
  user's responsibility (Vivado's own project locking degrades to
  read-only rather than corrupting).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-frontend`: the launcher's container naming and start-guard
  requirements change — per-mode container names, mode-aware
  idempotency/refusal, `stop --force` covering both containers, and an
  explicit requirement that GUI and headless sessions coexist in one
  workspace.

## Impact

- `scripts/dv` — container naming, `container_state`, `cmd_start`
  guard, `cmd_stop_force` (~20 lines).
- `scripts/smoke.davit.sh` — new coexistence test case (headless up +
  GUI name reserved + second GUI refused).
- `README.md` — note that a GUI can be opened alongside a running
  batch session.
- No changes to the container image, entrypoint, or the Rust daemon.
