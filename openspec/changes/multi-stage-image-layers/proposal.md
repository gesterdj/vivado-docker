## Why

Every change to the Docker image (new packages, stubs, entrypoint tweaks)
currently rebuilds the single-stage `docker/Dockerfile`, repeating the
~50GB installer handling and multi-hour Vivado install. Splitting into a
base image (install once) plus thin overlay images makes iteration cheap
and adopts the authenticated web-install pattern already specified in
`docs/fpgatools-docker-spec.md` §1, eliminating the local ~50GB archive
copy entirely.

## What Changes

- **BREAKING** Replace the single `docker/Dockerfile` with a layered
  build:
  - `docker/base/Dockerfile` — Ubuntu 22.04 + minimal installer
    prerequisites + batch web install of Vivado/Vitis (slim installer
    bind-mounted, pre-generated auth token mounted as a BuildKit
    secret). Produces `xilinx-vivado-base:<version>`. Rebuilt rarely.
  - `docker/tools/Dockerfile` — `FROM xilinx-vivado-base:<version>`;
    adds the libudev stub, extra dev packages, volumes, workdir, and any
    future customization. Produces `xilinx-vivado:<version>`. Rebuilds in
    minutes.
- **BREAKING** Install method switches from local ~50GB archive
  (`HOST_TOOL_ARCHIVE_NAME` tar bind-mount) to the AMD slim web installer
  (pre-unpacked `./Xilinx` dir bind-mount + host-generated auth token);
  xsetup downloads only the modules selected in the config.
- Add `scripts/gen_auth_token.sh` to support token generation: runs the
  downloaded slim installer with `<installer>.bin -- -b AuthTokenGen`,
  producing `~/.Xilinx/wi_authentication_key` on the host. The token is
  passed to the build as a BuildKit secret — credentials never enter the
  build.
- Use `config/install_config.txt` (installer-generated Vitis Unified
  config: Vitis platform with Artix-7, Zynq-7000, Zynq UltraScale+
  MPSoC, and Kria SOM/K26 device support) as the base install config,
  keeping the installer's native naming scheme.
- Makefile: new `build-base` and `build` targets with proper dependency
  ordering; remove `HOST_TOOL_ARCHIVE_NAME` plumbing.
- README/AGENTS: document the two-stage build flow and credentials
  handling.

## Capabilities

### New Capabilities
- `base-image`: Build contract for the rarely-rebuilt Vivado/Vitis base
  image (platform, packages, locale, authenticated batch install, tool
  paths).
- `layered-image`: Contract for overlay images extending the base
  (naming/tagging, what belongs in overlays vs base, rebuild-cost
  guarantee).

### Modified Capabilities
- `repo-layout`: `docker/` gains per-image subfolders
  (`docker/base/`, `docker/tools/`).

## Impact

- `docker/Dockerfile` split/replaced; `docker/udev_stub.c` moves to the
  tools overlay context.
- `Makefile`: target restructure; `build.stamp` semantics change.
- `config/`: installer-generated `install_config.txt` (Vitis Unified) is
  the only install config; the interim `xsetup_config_25.txt` removed.
- `scripts/run.vivado.sh`: unchanged behavior, but image tag source
  changes (`xilinx-vivado:<version>` now built from the tools overlay).
- Users must obtain AMD credentials and the slim installer instead of the
  full archive. Build secrets handling (credentials must not persist in
  image layers or history).
