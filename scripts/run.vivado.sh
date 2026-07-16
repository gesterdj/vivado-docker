#!/bin/bash
# Copyright 2023 Google. All rights reserved.
#
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Runs Vivado inside the Docker container. Supports both interactive (GUI/TCL)
# and batch modes.
#
# Environment variables:
#   VIVADO_VERSION  Vivado version (default: 2025.2)
#   SRC_DIR         Host directory to mount at /src (default: current dir)
#   WORK_DIR        Host directory to mount at /work (default: current dir)
#   VIVADO_CMD      Command to run (default: interactive Vivado GUI)
#                   Example for batch: VIVADO_CMD="vivado -mode batch -source /src/build.tcl"
#   USB_DEVICE_DIR  Host directory for mounted USB devices (default: no path)
#                   Override to enable USB support. USB Drivers needs to be installed on host.

set -euo pipefail
set -x

INTERACTIVE=()
if sh -c ": >/dev/tty" >/dev/null 2>/dev/null; then
	INTERACTIVE=(--interactive --tty)
fi

VIVADO_VERSION="${VIVADO_VERSION:-2025.2}"
VIVADO_PATH="/opt/Xilinx/${VIVADO_VERSION}/Vivado"

# Paths — override via environment if needed
SRC_DIR="${SRC_DIR:-$(pwd)}"
WORK_DIR="${WORK_DIR:-$(pwd)}"

# Add --device command only if USB path is provided
# (default for Ubuntu is /dev/bus/usb)
USB_DEVICE_DIR="${USB_DEVICE_DIR:-}"
USB_CMD=""
if [[ -n "${USB_DEVICE_DIR}" ]]; then
  USB_CMD="--device $USB_DEVICE_DIR"
fi

mkdir -p "${WORK_DIR}"

# Scope the universal libudev stub to the Vivado process tree. Vivado's
# license manager and WebTalk scan udev devices, which misbehaves in
# containers without a udev database.
PRELOAD_CMD="export LD_PRELOAD=/opt/udev_stub.so && "

# Default: interactive Vivado. Override VIVADO_CMD for batch mode.
# For batch synthesis: VIVADO_CMD="vivado -mode batch -source /src/build.tcl"
VIVADO_CMD="${VIVADO_CMD:-vivado}"

# Conditional docker flags for platform differences
DOCKER_ARGS=()
if [[ -d /tmp/.X11-unix ]]; then
  DOCKER_ARGS+=(-v /tmp/.X11-unix:/tmp/.X11-unix:ro)
fi
if [[ "$(uname -s)" == "Linux" ]]; then
  DOCKER_ARGS+=(--net=host)
fi

docker run \
  --platform linux/amd64 \
  "${INTERACTIVE[@]}" \
  --rm \
  -u "$(id -u):$(id -g)" \
  "${DOCKER_ARGS[@]}" \
  -v "${SRC_DIR}:/src:rw" \
  -v "${WORK_DIR}:/work:rw" \
  -e HOME="/work" \
  -e DISPLAY="${DISPLAY:-}" \
  -e _JAVA_AWT_WM_NONREPARENTING=1 \
  -e XILINX_LOCAL_USER_DATA=no \
  ${USB_CMD} \
  "xilinx-vivado:${VIVADO_VERSION}" \
  /bin/bash -c \
    "${PRELOAD_CMD}source ${VIVADO_PATH}/settings64.sh && cd /work && ${VIVADO_CMD}"
