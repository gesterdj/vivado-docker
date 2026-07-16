#!/bin/bash
# Builds the Vivado/Vitis base image (xilinx-vivado-base:<version>).
# Slow (multi-hour web install); rebuilt rarely. Run from anywhere —
# the build context is always the repo root.
#
# Environment variables:
#   VIVADO_VERSION   Vivado version tag (default: 2025.2)
#   INSTALLER        Path to the AMD slim installer .bin, relative to the
#                    repo root (default: first *.bin at the repo root)
#   AUTH_TOKEN_FILE  Host auth token from scripts/gen_auth_token.sh
#                    (default: ~/.Xilinx/wi_authentication_key)
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO}"

VIVADO_VERSION="${VIVADO_VERSION:-2025.2}"
BASE_IMAGE="xilinx-vivado-base:${VIVADO_VERSION}"
AUTH_TOKEN_FILE="${AUTH_TOKEN_FILE:-${HOME}/.Xilinx/wi_authentication_key}"

# Auto-detect the slim installer at the repo root unless given.
if [[ -z "${INSTALLER:-}" ]]; then
    for f in *.bin; do
        [[ -f "$f" ]] && INSTALLER="$f" && break
    done
fi

if [[ ! -f "${AUTH_TOKEN_FILE}" ]]; then
    echo "No auth token at ${AUTH_TOKEN_FILE} — run" \
         "'make auth-token INSTALLER=<slim-installer.bin>' first" >&2
    exit 1
fi
if [[ -z "${INSTALLER:-}" || ! -f "${INSTALLER}" ]]; then
    echo "No installer found — place the AMD slim installer .bin at the" \
         "repo root or pass INSTALLER=<path> (inside the build context)" >&2
    exit 1
fi

exec env DOCKER_BUILDKIT=1 docker build \
    --platform linux/amd64 \
    -t "${BASE_IMAGE}" \
    --build-arg VIVADO_VERSION="${VIVADO_VERSION}" \
    --build-arg INSTALLER_BIN="${INSTALLER}" \
    --secret id=xilinx_token,src="${AUTH_TOKEN_FILE}" \
    -f docker/base/Dockerfile .
