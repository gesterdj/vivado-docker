# Design: add-davit-session-cli

## Context

The repo builds two images (`xilinx-vivado-base:2025.2` →
`xilinx-vivado:2025.2`) and ships `scripts/run.vivado.sh` for one-shot
GUI/batch runs with `docker run -u UID:GID`. `docs/vivado-vitis-cli.md`
is the behavioral target for a persistent session + CLI;
`docs/fpgatools-docker-spec.md` documents a prior (gosu/SSH/pexpect)
incarnation that is prior art, not the target. Primary deployment shape
is a **sidecar**: an app/dev container and the Vivado session container
share the project directory mounted at `/workspace`.

Constraints: two images max, shallow spec slicing, today's non-root
`-u UID:GID` model, headless build environment, no Apple/Rosetta
support (udev stub retained as universal), target version 2025.2.

## Goals / Non-Goals

**Goals:**

- One change delivering session daemon + CLI, expanding only the tools
  overlay Dockerfile.
- Zero-dependency consumption from host and sibling containers.
- Conformance with `docs/vivado-vitis-cli.md` under the `-u UID:GID`
  model (its UID-alignment and SSH sections are superseded by decisions
  below).

**Non-Goals:**

- SSH transport, TCP listeners, multiple sessions per container.
- Changing `docker/base/Dockerfile` or the install flow.
- Podman support; USB/JTAG passthrough; VNC/Xpra.
- Removing `scripts/run.vivado.sh` (kept for one-shot use).

## Decisions

### D1: Single static Rust binary, two personalities

`davit/` is one Rust crate producing one
`x86_64-unknown-linux-musl` static binary. Invoked as `dv <verb>` it is
the client; the entrypoint invokes the hidden `dv _daemon` mode.
Rationale: static musl = zero runtime deps on any caller (alpine,
distroless, host); one artifact eliminates client/daemon protocol skew.
Alternatives: Python stdlib single file (rejected: requires python3 on
every caller), shell+socat (rejected: fragile framing), Go (user chose
Rust). Key crates: `nix`/`rustix` (PTY, procfs), `serde_json`
(protocol/artifacts), stdlib `UnixListener`.

### D2: Session root in the shared workspace

All session state lives in `<workspace>/.dv/`: `control.sock`,
`metadata.json`, `result.json`, `health.json`, `session-<ts>.log`,
`bin/dv`. The daemon publishes its own binary to `.dv/bin/dv` at
startup (self-publishing). Sidecars that mount the workspace get the
CLI, the socket, and all read-only artifacts with no installation.
Unix-socket-on-bind-mount is reliable because Apple support is dropped
(Linux hosts only). Alternative (named volume for the socket) rejected
as unnecessary complexity.

### D3: Runtime-verb / session-verb split

Only `start` (and the `--force` escalation of `stop`) need a container
runtime. `scripts/dv` is a thin bash launcher owning exactly those; all
other verbs `exec` into `.dv/bin/dv`. Graceful `stop` is a socket
request — daemon shuts down Vivado, exits, container exits. Sidecars
therefore have full control minus container creation, which belongs to
the orchestrator (compose `up`, `depends_on: service_healthy` gated by
the image `HEALTHCHECK` that runs `dv show health`).

### D4: Keep `-u UID:GID`; no root phase

Launcher/orchestrator must pass `-u UID:GID` (launcher derives it from
`id`; compose sets `user:`). Entrypoint verifies it is non-root and
that `/workspace` is writable, creates `.dv/`, and fails fast
otherwise. Host file ownership is automatic. Supersedes the spec's
root-entrypoint/gosu UID-alignment section; SSH is dropped (spec §11
not implemented) since the shared-volume socket serves the sidecar
case SSH was meant for.

### D5: Daemon architecture

Single-threaded-per-concern supervisor: PTY owner task reads Vivado
output continuously (raw log always appended); socket accept loop
serializes commands (busy → immediate structured rejection); result
latch via temp-file + atomic rename, reset to a `no completed command`
marker at dispatch; health sampler every 10 s from `/proc`. Client
timeout is client-side only — daemon always runs commands to
completion. `run <tool>` operations are daemon-spawned children,
registered in metadata, streamed to the caller, recorded in the raw
log; one scheduler serializes TCL and tool operations.

### D6: GUI mode stays simple

`dv start gui` runs `vivado -mode gui` as the foreground app — no
daemon, no socket, no `.dv` session artifacts beyond a mode marker in
metadata for `show status`. X11/Wayland/WSL profile detection per spec;
udev stub `LD_PRELOAD` applies to Vivado processes in both modes.

### D7: Dockerfile shape

`docker/tools/Dockerfile` gains a `FROM rust:*-alpine AS davit-builder`
stage (BuildKit-cached; toolchain never in final image) and the final
stage copies `/opt/davit/dv`, sets an `ENTRYPOINT` script handling
`session|gui` commands, and a `HEALTHCHECK`. Image count remains two;
default `docker run` behavior for existing users is preserved.

## Risks / Trade-offs

- [Vivado prompt matching on PTY is brittle] → exact-prompt matcher,
  all raw bytes logged; `diagnose` probes (procfs-only, never touch
  socket/PTY) for wedge analysis; heuristic documented, never auto-kill.
- [`.dv/` inside the workspace pollutes projects] → single dotted dir;
  document adding `.dv/` to project `.gitignore`; `dv start` prints
  the hint.
- [Socket file left stale after crash] → metadata + connect probe
  distinguish `unreachable` vs `crashed`; `start` refuses to adopt a
  foreign live session, replaces artifacts only when the container is
  provably gone.
- [Rust builder stage cost] → cached stage, crate is small; recurring
  cost seconds.
- [Non-root user lacks passwd entry under arbitrary UID] → already the
  proven model of `run.vivado.sh` (`HOME=/work`); daemon sets
  `HOME=/workspace`, avoids tools needing NSS lookups.
- [1 MiB result cap may truncate large reports] → explicit truncation
  marker; raw log uncapped; `logs` reads files directly.

## Migration Plan

Additive: existing `make build` users get a larger tools image;
`run.vivado.sh` one-shot flow unchanged except `ROSETTA` removal
(breaking only for Apple hosts, which are dropped deliberately).
Rollback = rebuild previous tools image; base image untouched.

## Open Questions

- Warning-suppression file name: spec says workspace `elfws.yaml` —
  adopt as-is or rename to `.dv/suppress.yaml`? (default: adopt spec
  name for conformance).
- Minimum protocol-version handshake fields in `metadata.json` (decide
  during implementation; must be present from v1).
