# syntax=docker/dockerfile:1
# Tools overlay: extends the Vivado/Vitis base image with developer
# packages, the universal libudev stub, the daVit session binary, and
# runtime volumes. Rebuilds in minutes — put environment customization
# here, not in docker/base/.
#
# Build (from repo root):
#   make build

ARG VIVADO_VERSION=2025.2

# --- daVit builder stage -----------------------------------------------
# Builds the static `dv` binary (client + session daemon). The Rust
# toolchain stays in this stage; only the musl-linked binary is copied
# into the final image.
FROM rust:alpine AS davit-builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY davit/Cargo.toml davit/Cargo.lock ./
COPY davit/src ./src
RUN cargo build --release --target x86_64-unknown-linux-musl \
    && strip target/x86_64-unknown-linux-musl/release/dv

# --- Final overlay -----------------------------------------------------
# Target platform (linux/amd64) is set by the Makefile's docker build
# --platform flag; do not hardcode it here.
FROM xilinx-vivado-base:${VIVADO_VERSION}

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

# Build the universal libudev stub.
# Vivado's license manager and WebTalk call udev_enumerate_scan_devices(),
# which misbehaves in containers without a udev database (crashes such as
# "realloc(): invalid pointer" / "mremap_chunk(): invalid pointer").
# This stub provides no-op implementations that return empty device lists.
# The daemon and entrypoint scope it to the Vivado/Vitis process tree via:
#   LD_PRELOAD=/opt/udev_stub.so vivado ...
COPY docker/udev_stub.c /tmp/udev_stub.c
RUN gcc -shared -fPIC -o /opt/udev_stub.so /tmp/udev_stub.c \
    && rm /tmp/udev_stub.c

# daVit session binary and entrypoint.
COPY --from=davit-builder /build/target/x86_64-unknown-linux-musl/release/dv /opt/davit/dv
COPY docker/tools/entrypoint.sh /opt/davit/entrypoint.sh
RUN chmod 0755 /opt/davit/dv /opt/davit/entrypoint.sh

VOLUME /src
VOLUME /work

WORKDIR /work

ENTRYPOINT ["/opt/davit/entrypoint.sh"]
CMD ["session"]

# Readiness: exits 0 only when the headless session daemon is up and the
# Vivado prompt has been seen. Compose siblings gate on service_healthy.
HEALTHCHECK --interval=10s --timeout=5s --start-period=600s --retries=3 \
    CMD ["/opt/davit/dv", "_healthcheck"]
