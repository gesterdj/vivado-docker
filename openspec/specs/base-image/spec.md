# base-image Specification

## Purpose
TBD - created by archiving change multi-stage-image-layers. Update Purpose after archive.
## Requirements
### Requirement: Base platform and installer prerequisites
The base image (`docker/base/Dockerfile`) SHALL build from
`ubuntu:22.04` with `DEBIAN_FRONTEND=noninteractive` and SHALL install,
via apt with retries (`Acquire::Retries=3`) and
`--no-install-recommends`, only the toolchain and runtime libraries
required by the Vivado/Vitis installer and tools (per
`docs/fpgatools-docker-spec.md` §1), cleaning apt caches in the same
layer. Developer convenience packages SHALL NOT be installed in the base
image.

#### Scenario: Minimal apt layer
- **WHEN** the base image is built
- **THEN** the apt layer completes with retries enabled, no recommended
  packages, and no residual apt list cache

### Requirement: UTF-8 locale
The base image SHALL generate the `en_US.UTF-8` locale and export
`LANG=en_US.UTF-8` and `LC_ALL=en_US.UTF-8`.

#### Scenario: Locale active
- **WHEN** a container from the base image runs `locale`
- **THEN** `LANG` and `LC_ALL` report `en_US.UTF-8`

### Requirement: Host-side auth token generation script
The repository SHALL provide `scripts/gen_auth_token.sh` which runs the
downloaded AMD slim installer binary with `-- -b AuthTokenGen` (AMD's
own interactive login), verifies that
`$HOME/.Xilinx/wi_authentication_key` was created, and reports the token
path. The script SHALL accept the installer path as an argument and
SHALL fail with a clear message when the installer is missing or the
token was not produced.

#### Scenario: Token generated
- **WHEN** the operator runs
  `scripts/gen_auth_token.sh ./FPGAs_..._Web.bin` and completes AMD's
  login prompt
- **THEN** the script confirms `~/.Xilinx/wi_authentication_key` exists
  and exits zero

#### Scenario: Installer missing
- **WHEN** the script is invoked with a nonexistent installer path
- **THEN** it exits non-zero naming the missing file, without prompting

### Requirement: Batch web install using token secret
The base image SHALL install Vivado/Vitis in unattended batch mode:

- The slim installer `.bin` SHALL be provided via a read-only BuildKit
  bind mount, self-extracted within the install RUN, with all extraction
  temp files removed in the same layer; installer content SHALL never be
  copied into an image layer.
- The host auth token SHALL be provided via a BuildKit secret mount
  (`id=xilinx_token`) at the path xsetup expects; it SHALL NOT be passed
  as a build arg, COPY'd, or persisted in any layer.
- The build SHALL run xsetup with `--batch Install`, the config from
  `config/install_config.txt`, and
  `--agree XilinxEULA,3rdPartyEULA`.
- Post-install, the build SHALL remove installer staging and download
  caches in the same layer.

#### Scenario: No credentials or token in image
- **WHEN** the built base image is inspected with `docker history` and
  filesystem search
- **THEN** no AMD credentials or `wi_authentication_key` content appear
  in any layer or metadata

#### Scenario: Batch install completes without interaction
- **WHEN** the base image is built with a valid token secret and a
  populated installer bind mount
- **THEN** xsetup completes in batch mode with both EULAs agreed and no
  staging or download caches remain in the final layer

### Requirement: Tool paths and default command
The base image SHALL set `VIVADO_PATH` and `VITIS_PATH` for the installed
version, prepend their `bin` directories to `PATH`, set `WORKDIR
/workspace`, and default to `CMD ["vivado", "-mode", "batch"]`.

#### Scenario: Tools on PATH
- **WHEN** a container starts from the base image with no command
- **THEN** `vivado -mode batch` launches from `/workspace` and both
  `vivado` and Vitis binaries resolve on `PATH`

