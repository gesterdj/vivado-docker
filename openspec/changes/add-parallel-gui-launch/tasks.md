# Tasks: add-parallel-gui-launch

## 1. Launcher changes (scripts/dv)

- [ ] 1.1 Introduce per-mode container names: keep `CONTAINER`
      (`davit-<wshash>`) for headless, add `CONTAINER_GUI`
      (`davit-<wshash>-gui`); parameterize `container_state` by name
- [ ] 1.2 Make the `cmd_start` guard mode-aware: resolve the target
      name from the mode before the state check; keep headless
      idempotency; refuse a duplicate GUI with "gui already open" and
      non-zero exit; remove stale owned containers per mode
- [ ] 1.3 Add label `davit.session=gui` to the GUI `docker run` and
      use `CONTAINER_GUI` as its `--name`
- [ ] 1.4 Extend `cmd_stop_force` to sweep both names (stop + remove
      each owned container that exists; error only if neither exists;
      report each removal)

## 2. Verification

- [ ] 2.1 Add a coexistence case to `scripts/smoke.davit.sh`: with the
      headless session running, assert the GUI name path is free
      (start-guard passes for gui), a second GUI is refused, `dv exec`
      still works, and `stop --force` removes both containers
- [ ] 2.2 Run the smoke test (or, without a runtime/image, review the
      launcher paths with `bash -n` and targeted dry checks)

## 3. Documentation

- [ ] 3.1 Update README: GUI can be opened alongside a running batch
      session; concurrent project access is the user's responsibility;
      `stop --force` cleans up both containers
