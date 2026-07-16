# Proposal: add-davit-session-cli

## Why

`scripts/run.vivado.sh` only supports one-shot GUI/batch runs: no
persistent TCL session, no status/result surface, no way for a sibling
container or CI agent to drive Vivado/Vitis. `docs/vivado-vitis-cli.md`
specifies the desired behavior; this change realizes it as **daVit**
(CLI: `dv`), layered on the existing two-image build.

## What Changes

- Add `davit`, a static Rust (musl) binary combining the in-container
  session daemon and the `dv` client CLI, built in a cached multi-stage
  builder stage of `docker/tools/Dockerfile` (image count stays at two).
- Persistent headless Vivado TCL session: daemon owns Vivado via PTY,
  serializes commands, latches results atomically, filters output
  (INFO/WARNING suppression with `elfws.yaml` overrides), samples
  process-tree health, appends an unfiltered raw log.
- Session root at `<workspace>/.dv/`: Unix control socket, metadata,
  result, health, raw logs, and the self-published `dv` binary
  (`.dv/bin/dv`) copied by the entrypoint at startup — sidecar
  containers sharing the workspace mount need zero preinstalled
  dependencies.
- `dv` verb grammar per `docs/vivado-vitis-cli.md`: `start`, `stop`,
  `exec`, `show status|result|metadata|health`, `logs`, `diagnose`,
  `run xsct|xsdb|bootgen|dtc`; `--json` output, stable exit codes, no
  wildcard TCL fallthrough.
- Thin host launcher `scripts/dv` owning only runtime verbs (`start`
  creates the container with `-u UID:GID`; `stop --force` runtime
  escalation); every other verb delegates to the published binary.
- GUI mode as a mutually exclusive container mode (X11/Wayland/WSLg
  profiles, no daemon/socket); `--jtag-host` host `hw_server` brokering.
- Image `HEALTHCHECK` reflecting session readiness so orchestrators can
  gate siblings with `depends_on: service_healthy`.
- Containers keep today's non-root `-u UID:GID` model: no root
  entrypoint, no gosu, no chown. SSH transport is dropped entirely; the
  shared-volume socket is the single transport.
- **BREAKING** Remove Apple Silicon/Rosetta support repo-wide
  (`ROSETTA` env in `scripts/run.vivado.sh`, README Apple Silicon
  section). The libudev stub stays as a universal Vivado-in-Docker
  mitigation, no longer Apple-branded.
- New `davit/` top-level folder for the Rust crate.

## Capabilities

### New Capabilities

- `vivado-session`: in-container session daemon contract — lifecycle,
  PTY ownership, session-root artifacts, socket protocol, result
  semantics, output filtering, health sampling, tool operations, GUI
  mode exclusivity, udev-stub scoping, JTAG env.
- `cli-frontend`: public `dv` command grammar — verbs, options, JSON
  shapes, exit codes, host launcher vs published-binary split,
  self-publishing and discovery, sidecar usage.

### Modified Capabilities

- `layered-image`: tools overlay gains a Rust builder stage, the
  `davit` binary, entrypoint and `HEALTHCHECK`; udev-stub requirement
  reworded as universal (not Rosetta-specific).
- `repo-layout`: add `davit/` (Rust crate) to the canonical folder
  structure; `scripts/` gains the `dv` launcher.
- `docs-structure`: README drops the Apple Silicon section and the
  `ROSETTA` variable; gains daVit session/CLI usage documentation.

## Impact

- `docker/tools/Dockerfile`: multi-stage Rust builder, binary install,
  entrypoint, `HEALTHCHECK`.
- New `davit/` crate (daemon + client in one binary) and `scripts/dv`
  launcher; `scripts/run.vivado.sh` loses `ROSETTA` handling but keeps
  one-shot behavior.
- `README.md`, `AGENTS.md`: document `dv` usage, sidecar compose
  pattern; remove Apple Silicon content.
- No change to `docker/base/Dockerfile` or the install flow; base image
  untouched. Target version stays 2025.2.
- Toolchain: Rust (musl target) required at image build time only —
  confined to a builder stage, never in the final image.
