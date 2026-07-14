# syntax=docker/dockerfile:1
# Tools overlay: extends the Vivado/Vitis base image with developer
# packages, the Rosetta libudev stub, and runtime volumes. Rebuilds in
# minutes — put environment customization here, not in docker/base/.
#
# Build (from repo root):
#   make build

ARG VIVADO_VERSION=2025.2
# hadolint ignore=DL3029
FROM --platform=linux/amd64 xilinx-vivado-base:${VIVADO_VERSION}

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Developer package set (carried over from the former single-stage image).
# hadolint ignore=DL3008
RUN apt-get -o Acquire::Retries=3 update \
    && apt-get -o Acquire::Retries=3 install -y --no-install-recommends \
        autoconf \
        chrpath \
        cpio \
        default-jre \
        diffstat \
        dos2unix \
        fakeroot \
        gawk \
        gcc-multilib \
        gnupg \
        google-perftools \
        gzip \
        iproute2 \
        less \
        libglib2.0-dev \
        libgtk2.0-dev \
        libncurses5-dev \
        libsdl1.2-dev \
        libselinux1 \
        libssl-dev \
        libtool \
        libtool-bin \
        make \
        net-tools \
        pax \
        python3-gi \
        rsync \
        screen \
        socat \
        tar \
        texinfo \
        tofrodos \
        xterm \
        xvfb \
        zlib1g-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Build libudev stub for Rosetta compatibility.
# Vivado's license manager and WebTalk call udev_enumerate_scan_devices()
# which crashes under Rosetta x86_64 emulation with:
#   "realloc(): invalid pointer" / "mremap_chunk(): invalid pointer"
# This stub provides no-op implementations that return empty device lists.
# Use via: LD_PRELOAD=/opt/udev_stub.so vivado ...
# On native x86_64 hosts this stub is harmless but unnecessary.
COPY docker/udev_stub.c /tmp/udev_stub.c
RUN gcc -shared -fPIC -o /opt/udev_stub.so /tmp/udev_stub.c \
    && rm /tmp/udev_stub.c

# Install Xilinx USB cable drivers (for USB_DEVICE_DIR passthrough).
# hadolint ignore=DL3003
RUN mkdir -p /etc/udev/rules.d \
    && cd "${VIVADO_PATH}/data/xicom/cable_drivers/lin64/install_script/install_drivers" \
    && ./install_drivers

VOLUME /src
VOLUME /work

WORKDIR /work
