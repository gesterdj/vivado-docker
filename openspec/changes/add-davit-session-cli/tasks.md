# Tasks: add-davit-session-cli

## 1. Crate scaffold and protocol

- [ ] 1.1 Create `davit/` Rust crate (musl target config, `serde_json`,
      `nix`/`rustix`, clap-style argv parsing without eval) producing
      one `dv` binary with client verbs and hidden `_daemon` mode
- [ ] 1.2 Define session artifact schemas (`metadata.json`,
      `result.json`, `health.json`) with protocol version field and
      atomic temp-file+rename writer
- [ ] 1.3 Define framed Unix-socket request/response protocol with
      malformed/oversized request rejection

## 2. Daemon core

- [ ] 2.1 Implement PTY ownership of `vivado -mode tcl`: spawn, exact
      prompt matcher, continuous raw-log append, readiness detection
      (prompt + optional project open)
- [ ] 2.2 Implement serialized command scheduler: single in-flight
      operation, immediate structured `busy` rejection, result latch
      (no-result marker on dispatch, atomic result on completion,
      1 MiB cap with truncation marker)
- [ ] 2.3 Implement stateful output filter (INFO/WARNING suppression,
      continuation handling, `elfws.yaml` per-command reload with safe
      fallback on parse failure)
- [ ] 2.4 Implement health sampler (10 s procfs sweep: descendants,
      CPU %, RSS, last PTY read) and graceful-shutdown socket request
      (refuse while busy)
- [ ] 2.5 Implement `run xsct|xsdb|bootgen|dtc` operations: metadata
      registration, Vitis env init, xsct cwd rule, verbatim
      argv/stdio/exit-status passthrough, raw-log operation headers,
      shared scheduler serialization
- [ ] 2.6 Publish self binary to `.dv/bin/dv` at startup; set
      owner-only socket permissions; fail fast on root UID or
      unwritable workspace

## 3. Client verbs

- [ ] 3.1 Implement `exec` (inline/`--file`, `--timeout` exit 3
      pointing to `show result`, exit-code contract 0/1/2/3, Vivado
      death vs TCL error distinction on stderr)
- [ ] 3.2 Implement `show status|result|metadata|health` and
      `logs --tail/--follow` from artifacts only; `unreachable`
      detection without state rewrite; `--json` single-value output
- [ ] 3.3 Implement `diagnose last|metadata|health|inspect|logs|ps|
      wchan|fionread|fdtable` from artifacts and procfs only (never
      socket/PTY), JSON with explicit nulls
- [ ] 3.4 Implement graceful `stop` over the socket and clear
      "lifecycle owned by orchestrator" errors for runtime verbs when
      no runtime is available

## 4. Image and entrypoint

- [ ] 4.1 Add Rust musl builder stage to `docker/tools/Dockerfile`;
      install `/opt/davit/dv`; keep toolchain out of the final image
- [ ] 4.2 Add entrypoint dispatching `session`/`gui` modes (GUI: no
      daemon/socket; udev stub LD_PRELOAD scoped to Vivado/Vitis
      process trees in both modes) and `HEALTHCHECK` running readiness
      check
- [ ] 4.3 Update `Makefile`/`.dockerignore` as needed so `make build`
      builds the crate stage from `davit/`

## 5. Host launcher

- [ ] 5.1 Create `scripts/dv`: `start` (input validation, `.xpr`
      checks, `docker run --init -u UID:GID -v <ws>:/workspace`,
      idempotent duplicate start, readiness wait), `stop --force`
      runtime escalation, delegation of all other verbs to
      `.dv/bin/dv`, hint+exit 2 when no session root
- [ ] 5.2 Implement GUI profile detection (wsl/wayland/x11,
      `--gui-profile` override, read-only display mounts) and
      `--jtag-host` resolution/probe/`VIVADO_HW_SERVER_URL` plumbing

## 6. Apple support removal

- [ ] 6.1 Remove `ROSETTA` handling from `scripts/run.vivado.sh`
      (keep one-shot behavior otherwise)
- [ ] 6.2 Remove Apple Silicon/Rosetta content from `README.md`;
      re-describe the udev stub as universal Vivado-in-Docker
      mitigation (Dockerfile comments included)

## 7. Documentation

- [ ] 7.1 README: daVit usage (verbs, `.dv/` session root, exit codes,
      suppression file), sidecar compose example with
      `service_healthy`, updated env-var table, updated repository
      layout section with `davit/`
- [ ] 7.2 Update `AGENTS.md` links/constraints if affected; add
      `.dv/` to `.gitignore` guidance

## 8. Validation

- [ ] 8.1 Rust unit tests: filter state machine, result latch/cap,
      busy rejection, protocol framing, argv preservation with
      metacharacters
- [ ] 8.2 Container smoke test (requires built image): start headless,
      exec success/error/busy/timeout, show/logs/diagnose surfaces,
      run bootgen exit-code passthrough, graceful stop, host file
      ownership
- [ ] 8.3 Sidecar smoke test: compose two services sharing a
      workspace, `service_healthy` gating, `.dv/bin/dv` usage from
      the sibling with no preinstalled deps
