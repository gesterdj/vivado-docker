#!/bin/bash
# daVit container entrypoint: dispatches the two session modes.
#
#   session [--project <file.xpr>]   headless daemon (foreground app)
#   gui [vivado args...]             vivado -mode gui (foreground app)
#
# The container must run with `-u UID:GID` (non-root) and `--init`
# (PID 1 reaping/signal forwarding is owned by the runtime, not us).
# Anything else is passed through verbatim (e.g. `bash` for debugging).
set -euo pipefail

if [[ "$(id -u)" -eq 0 ]]; then
    echo "davit: refusing to run as root; start the container with -u UID:GID" >&2
    exit 1
fi

mode="${1:-session}"
shift || true

case "$mode" in
    session)
        if [[ ! -w /workspace ]]; then
            echo "davit: /workspace is missing or not writable by UID $(id -u)" >&2
            exit 1
        fi
        exec /opt/davit/dv _daemon --workspace /workspace "$@"
        ;;
    gui)
        # GUI mode: no daemon, no socket, no session artifacts. The udev
        # stub is scoped to the Vivado process tree via LD_PRELOAD.
        exec env LD_PRELOAD=/opt/udev_stub.so vivado -mode gui "$@"
        ;;
    *)
        exec "$mode" "$@"
        ;;
esac
