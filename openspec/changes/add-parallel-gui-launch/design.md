# Design: add-parallel-gui-launch

## Context

`scripts/dv` derives one deterministic container name per workspace
(`davit-$(cksum <workspace>)`) and uses it for both `start headless`
and `start gui`. `cmd_start` checks that single name before branching
on mode, so a GUI launch while a headless session runs is rejected
with "session already running" — and would collide on `--name` even
without the guard. GUI mode is deliberately session-free (no daemon,
no socket, no `.dv/` writes; see `docker/tools/entrypoint.sh`), so the
only shared resources are the container name and the workspace mount.

The target workflow is batch-first: a persistent headless session
serves `dv exec` (agents, scripts) while a human occasionally opens a
GUI on the same workspace to read reports, placements, and schematics.

## Goals / Non-Goals

**Goals:**
- `dv start gui` succeeds while a headless session is running, and
  vice versa.
- GUI containers are identifiable and cleaned up by `dv stop --force`.
- Zero migration: headless container name, `.dv/` layout, daemon,
  image, and entrypoint are untouched.

**Non-Goals:**
- Multiple named headless sessions (`-main-<suffix>` namespacing,
  `.dv/sessions/<name>/`, named dispatch). Future change if ever
  needed.
- A control socket for the GUI (GUI-as-session). GUI stays invisible
  to `exec`/`show`/`logs`/`diagnose`/`run`.
- Guarding concurrent `.xpr` access. Vivado's own project locking
  applies; avoiding conflicting writes is the user's responsibility.
- More than one simultaneous GUI per workspace.

## Decisions

1. **Suffix only the GUI name** — `CONTAINER_GUI="davit-<wshash>-gui"`,
   label `davit.session=gui` added alongside the existing workspace
   label. Alternative considered: rename headless to `-main` for
   symmetry. Rejected: breaks running sessions, docs, and the smoke
   test for no functional gain; symmetry can arrive with a future
   multi-session change.

2. **Mode-aware guard, same semantics per mode** —
   `container_state <name>` gains a name parameter. `cmd_start`
   resolves the target name from the mode first, then applies the
   existing logic (workspace-label ownership check, stale-container
   removal, refusal/idempotency) against that name only. Headless
   keeps its idempotent "report status, exit 0" path; GUI refuses a
   duplicate with exit 1 ("gui already open") since there is no status
   to show and two investigation windows are not a supported state.

3. **`stop --force` sweeps both names** — force-stop iterates over
   `davit-<wshash>` and `davit-<wshash>-gui`, stopping/removing each
   that exists and is owned by this workspace; it errors only when
   neither exists. Alternative: `dv stop gui` verb. Rejected as scope
   creep — GUI normally exits with window close (`--rm`); force is the
   escape hatch for a hung X11 client.

4. **No entrypoint/daemon changes** — the GUI path already runs
   `IMAGE gui` as a foreground `docker run --rm` with `exec`; only the
   `--name` value changes.

## Risks / Trade-offs

- [Both sessions write `$HOME=/workspace` state (`.Xil/`, journals)] →
  Vivado's per-process journal/lock naming plus the read-only nature
  of GUI investigation makes clashes benign; documented as user
  responsibility in README.
- [Stale stopped GUI container blocks the name] → same stale-removal
  path headless already has: owned + not running → `rm -f`, proceed.
- [`stop --force` killing the GUI mid-look surprises the user] → the
  existing "in-flight results may be lost" warning is printed; sweep
  order (headless then gui) is deterministic and both are reported.

## Migration Plan

None required. Existing headless sessions keep their name; first GUI
launch after upgrade simply uses the new suffixed name. Rollback is a
script revert.

## Open Questions

None.
