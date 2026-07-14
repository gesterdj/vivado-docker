## 1. Auth Token Support

- [x] 1.1 Create `scripts/gen_auth_token.sh`: accept slim installer
      `.bin` path as `$1`, validate it exists and is executable, run
      `<installer> -- -b AuthTokenGen`, then verify
      `$HOME/.Xilinx/wi_authentication_key` exists; clear errors on
      missing installer or missing token; `chmod +x`

## 2. Base Image

- [x] 2.1 Create `docker/base/Dockerfile`: `ubuntu:22.04`, noninteractive
      apt with `Acquire::Retries=3` + `--no-install-recommends`,
      spec §1 minimal package list, apt cache cleanup in same layer
- [x] 2.2 Add locale layer (`en_US.UTF-8`, `LANG`/`LC_ALL` env)
- [x] 2.3 Add install layer: bind-mount installer dir at
      `/opt/xilinx_installer` (read-only), secret mount `xilinx_token`
      at xsetup's expected token path, copy
      `config/install_config.txt` as xsetup config, run
      `xsetup --batch Install --agree XilinxEULA,3rdPartyEULA`, remove
      staging/download caches in same layer
- [x] 2.4 Set `VIVADO_PATH`/`VITIS_PATH`, prepend both `bin` dirs to
      `PATH`, `WORKDIR /workspace`, `CMD ["vivado","-mode","batch"]`

## 3. Tools Overlay Image

- [x] 3.1 Create `docker/tools/Dockerfile`: `FROM
      xilinx-vivado-base:${VIVADO_VERSION}` (build-arg), install the
      developer package set from the old single-stage Dockerfile
- [x] 3.2 Compile `docker/udev_stub.c` to `/opt/udev_stub.so` (path
      unchanged for `run.vivado.sh`)
- [x] 3.3 Declare `VOLUME /src`, `VOLUME /work`, `WORKDIR /work`
- [x] 3.4 Delete old `docker/Dockerfile`

## 4. Makefile

- [x] 4.1 Add `auth-token` target invoking `scripts/gen_auth_token.sh`
- [x] 4.2 Add `build-base`/`base.stamp` target: docker build of
      `docker/base/Dockerfile` tagged `xilinx-vivado-base:${VIVADO_VERSION}`
      with installer bind mount and `--secret id=xilinx_token,src=...`;
      fail fast with `make auth-token` hint when token file missing;
      skip build only if stamp is current AND `docker image inspect
      xilinx-vivado-base:${VIVADO_VERSION}` succeeds (guards against
      stale stamps after `docker rmi`)
- [x] 4.3 Rework `build`/`build.stamp`: depends on `base.stamp`,
      `docker/tools/Dockerfile`, `docker/udev_stub.c`; tags
      `xilinx-vivado:${VIVADO_VERSION}`; same image-existence guard via
      `docker image inspect`
- [x] 4.4 Remove `HOST_TOOL_ARCHIVE_NAME`/`HOST_TOOL_ARCHIVE_EXTENSION`
      plumbing; update `all` help text; keep `make run` unchanged
- [x] 4.5 Verify: `make` help runs; `make -n build-base` / `make -n
      build` show correct commands; grep for stale
      `HOST_TOOL_ARCHIVE_NAME`/old Dockerfile references

## 5. Documentation

- [x] 5.1 README: rewrite build section for the two-stage flow (download
      slim installer, `make auth-token`, `make build-base`, `make
      build`), token-secret security note, spec-deviation note
- [x] 5.2 README: update Repository layout table for `docker/base/`,
      `docker/tools/`; remove legacy `config/install_config.txt` and all
      archive-flow/edition mentions (override with the new flow)
- [x] 5.3 AGENTS.md: update constraints/links if affected
- [x] 5.4 Verify docs: no references to the archive flow as current;
      80-column check on new lines

## 6. Finalize

- [x] 6.1 Commit as `feat!: split image into base install and tools
      overlay layers` with Co-authored-by trailer
