# syntax=docker/dockerfile:1
# Base image: Vivado/Vitis batch web install per
# docs/fpgatools-docker-spec.md §1. Rebuilt rarely — keep this image
# minimal; developer tooling belongs in docker/tools/Dockerfile.
#
# Build (from repo root):
#   make build-base INSTALLER=<slim-installer.bin>
# Requires:
#   - the AMD slim (web) installer .bin at the repo root
#   - host auth token (~/.Xilinx/wi_authentication_key), see
#     scripts/gen_auth_token.sh — passed as BuildKit secret, never
#     stored in a layer.

# hadolint ignore=DL3029
FROM --platform=linux/amd64 ubuntu:22.04

ARG VIVADO_VERSION=2025.2
ARG INSTALL_TARGET_DIR=/opt/Xilinx
ARG INSTALLER_BIN

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Minimal installer prerequisites and tool runtime libraries (spec §1).
# hadolint ignore=DL3008
RUN apt-get -o Acquire::Retries=3 update \
    && apt-get -o Acquire::Retries=3 install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        expect \
        file \
        fontconfig \
        git \
        libglib2.0-0 \
        libgtk2.0-0 \
        libice6 \
        libncurses5 \
        libpng16-16 \
        libsm6 \
        libtinfo5 \
        libxext6 \
        libxft2 \
        libxi6 \
        libxrender1 \
        libxtst6 \
        locales \
        lsb-release \
        tcl-dev \
        udev \
        unzip \
        wget \
        xz-utils \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# UTF-8 locale (spec §1)
RUN locale-gen --purge en_US.UTF-8
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

# Authenticated batch web install.
# - The slim installer .bin is bind-mounted read-only, self-extracted
#   inside this layer, and its temp files removed in the same layer —
#   only the installed tools persist.
# - Auth token mounted as a BuildKit secret at the path xsetup expects;
#   it exists only for the duration of this RUN.
# - xsetup downloads only the modules selected in the config, installs,
#   and the download cache is removed in the same layer.
COPY config/install_config.txt /tmp/xilinx/xsetup_config.txt
RUN --mount=type=bind,source=${INSTALLER_BIN},target=/tmp/installer.bin \
    --mount=type=secret,id=xilinx_token,target=/root/.Xilinx/wi_authentication_key \
    sh /tmp/installer.bin --keep --noexec --target /tmp/installer \
    && /tmp/installer/xsetup \
        --batch Install \
        --agree XilinxEULA,3rdPartyEULA \
        -c /tmp/xilinx/xsetup_config.txt \
    && rm -rf /tmp/installer /tmp/xilinx "${INSTALL_TARGET_DIR}/Downloads"

# Tool paths and default command (spec §1; paths follow the unified
# installer layout /opt/Xilinx/<version>/<tool> used by run.vivado.sh)
ENV VIVADO_PATH=${INSTALL_TARGET_DIR}/${VIVADO_VERSION}/Vivado
ENV VITIS_PATH=${INSTALL_TARGET_DIR}/${VIVADO_VERSION}/Vitis
ENV PATH=${VIVADO_PATH}/bin:${VITIS_PATH}/bin:${PATH}

WORKDIR /workspace

CMD ["vivado", "-mode", "batch"]
