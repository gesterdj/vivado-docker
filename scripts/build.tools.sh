#!/bin/bash
# Builds the tools overlay image (xilinx-vivado:<version>) on top of the
# base image: developer packages, the udev stub, and the daVit session
# binary (built in a cached Rust musl stage). Fast — minutes, not hours.
#
# Environment variables:
#   VIVADO_VERSION  Vivado version tag (default: 2025.2)
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO}"

VIVADO_VERSION="${VIVADO_VERSION:-2025.2}"
BASE_IMAGE="xilinx-vivado-base:${VIVADO_VERSION}"
TOOLS_IMAGE="xilinx-vivado:${VIVADO_VERSION}"

if ! docker image inspect "${BASE_IMAGE}" >/dev/null 2>&1; then
    echo "Base image ${BASE_IMAGE} not found — build it first with" \
         "'make build-base' (or scripts/build.base.sh)" >&2
    exit 1
fi

exec env DOCKER_BUILDKIT=1 docker build \
    --platform linux/amd64 \
    -t "${TOOLS_IMAGE}" \
    --build-arg VIVADO_VERSION="${VIVADO_VERSION}" \
    -f docker/tools/Dockerfile .
