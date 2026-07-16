---
name: davit-fpga-tools
description: Drive AMD Vivado/Vitis through the daVit session CLI (dv) for synthesis, implementation, reports, and Vitis tool runs. Use when the user wants to run TCL against a Vivado project, build FPGA artifacts, or invoke xsct/xsdb/bootgen/dtc in this containerized environment.
license: MIT
compatibility: Requires the built xilinx-vivado image and a workspace with a session (or Docker to start one).
metadata:
  author: vivado-docker
  version: "1.0"
---

Operate Vivado and Vitis tools through daVit, the persistent session CLI.

**Before anything else: read the amendment file**

Check for `AMENDMENTS.md` next to this SKILL.md. If it exists, read it
and treat its entries as overriding corrections to this template —
they are lessons from real interactions that did not work immediately.
If it does not exist, proceed; you may create it later (see "Recording
lessons learnt").

## Tool model

- One warm Vivado TCL session per workspace lives in a container;
  Vivado's multi-minute startup is paid once, not per command.
- Session root is `<workspace>/.dv/`: control socket, `metadata.json`,
  `result.json`, `health.json`, raw session logs, and a self-published
  client at `.dv/bin/dv` (usable from sidecars with zero dependencies).
- Exactly one operation runs at a time. Concurrent calls are rejected
  with `busy` immediately — never queued. Do not retry in a tight loop;
  check `dv show status` first.

## Core commands

```bash
dv start [headless|gui] [--project FILE.xpr]   # host launcher only
dv exec [--timeout S] [--file F] [--] TCL...   # one TCL operation
dv show status|result|metadata|health [--json]
dv logs [--tail N] [--follow]
dv diagnose last|health|inspect|ps|wchan|fionread|fdtable
dv run xsct|xsdb|bootgen|dtc ARGS...           # Vitis/companion tools
dv stop [--force]                              # --force: host only
```

Exit codes: `0` ok · `1` Vivado/TCL error, busy, or crash · `2` usage
or no result · `3` client wait timed out (command keeps running).

## Ground rules

1. **Prefer `--file` for nontrivial TCL.** Inline args are joined with
   single spaces; multi-statement or brace-heavy scripts belong in a
   file (`dv exec --file build.tcl`).
2. **Long operations**: launch with a generous `--timeout` or none at
   all; on exit 3 the command is still running — poll `dv show status`
   and fetch the outcome with `dv show result` when idle again.
3. **Never kill the session on a hunch.** Use `dv show health` first:
   high descendant CPU with quiet PTY = active compute phase (normal
   for synth/route); zero descendants, zero CPU, and old PTY read =
   possible wedge. Ask the user before `stop --force`.
4. **Prefer in-process TCL** (`synth_design`, `opt_design`,
   `place_design`, `route_design`, `write_bitstream`) over
   `launch_runs` where practical; child processes are harder to
   observe and have caused udev-related crashes in containers.
5. **INFO/WARNING lines are filtered** from exec output by default;
   ERROR and CRITICAL WARNING always surface. Full unfiltered output
   is in `dv logs`. Retainable warning IDs go in
   `<workspace>/elfws.yaml`.
6. **xsct scripts**: `dv run xsct <tclfile>` runs with the TCL file's
   directory as cwd; keep relative paths in the script relative to
   the script itself.
7. **Hardware access** goes through a host `hw_server`
   (`VIVADO_HW_SERVER_URL`, default `host.docker.internal:3121`) —
   there is no USB passthrough. In TCL:
   `open_hw_manager; connect_hw_server -url $env(VIVADO_HW_SERVER_URL)`.
8. **Lifecycle**: only the host launcher (`scripts/dv`) may `start` or
   `stop --force`. From inside containers/sidecars use graceful
   `dv stop` (refuses while a command is in flight — that is by
   design, not an error to work around).

## Recording lessons learnt

When an interaction with the tools does **not** work on the first
attempt and you resolve it (wrong flag, unexpected output shape,
timing/readiness issue, quirky TCL behavior, environment gotcha):

1. Open (or create) `AMENDMENTS.md` in this skill's directory.
2. Append a concise entry, or **edit an existing entry** if it covers
   the same topic — keep the file deduplicated and short.
3. Use this format:

   ```markdown
   ## <one-line topic>
   - Symptom: <what failed / what you observed>
   - Fix: <the working invocation or approach>
   ```

**Scope limit — strictly tool use.** Amendments MUST concern direct
use of the tools themselves: `dv` invocation patterns, Vivado/Vitis
TCL behavior, xsct/xsdb/bootgen/dtc quirks, session/timing semantics,
environment variables. Do NOT record project-specific facts, design
decisions, file layouts, user preferences, or anything covered by
other memory mechanisms. Do not duplicate what this SKILL.md already
states — amend only where reality diverged from it.
