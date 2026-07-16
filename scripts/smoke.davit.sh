#!/bin/bash
# daVit smoke test — requires the built xilinx-vivado image and Docker.
#
#   ./scripts/smoke.davit.sh            container smoke test (tasks: start,
#                                       exec ok/error/busy/timeout, show,
#                                       logs, diagnose, run, stop, ownership)
#   ./scripts/smoke.davit.sh --sidecar  compose sidecar test (service_healthy
#                                       gating + .dv/bin/dv from a sibling)
#
# Environment: VIVADO_VERSION (default 2025.2), DV_IMAGE override.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
VIVADO_VERSION="${VIVADO_VERSION:-2025.2}"
IMAGE="${DV_IMAGE:-xilinx-vivado:${VIVADO_VERSION}}"

PASS=0 FAIL=0
check() { # check <name> <expected-rc> <actual-rc>
    if [[ "$2" == "$3" ]]; then
        echo "ok   $1"; PASS=$((PASS+1))
    else
        echo "FAIL $1 (expected rc=$2, got rc=$3)"; FAIL=$((FAIL+1))
    fi
}

docker image inspect "${IMAGE}" >/dev/null 2>&1 \
    || { echo "image ${IMAGE} not found — run 'make build' first" >&2; exit 1; }

WS="$(mktemp -d /tmp/davit-smoke.XXXXXX)"
trap 'DV_WORKSPACE=${WS} "${REPO}/scripts/dv" stop --force >/dev/null 2>&1 || true; rm -rf "${WS}"' EXIT
cd "${WS}"

# Minimal empty project via a bootstrap batch run (first installed part)
echo 'create_project smoke /workspace/smoke -part [lindex [get_parts] 0]; exit' \
    > "${WS}/bootstrap.tcl"
docker run --rm --init -u "$(id -u):$(id -g)" \
    -v "${WS}:/workspace" -w /workspace -e HOME=/workspace \
    --entrypoint bash "${IMAGE}" -lc \
    'LD_PRELOAD=/opt/udev_stub.so vivado -mode batch -nolog -nojournal \
     -source /workspace/bootstrap.tcl'
rm -f "${WS}/bootstrap.tcl"
ln -sf smoke/smoke.xpr smoke.xpr

# --- sidecar mode ------------------------------------------------------
if [[ "${1:-}" == "--sidecar" ]]; then
    cat > compose.yaml <<EOF
services:
  vivado:
    image: ${IMAGE}
    init: true
    user: "$(id -u):$(id -g)"
    command: ["session", "--project", "smoke.xpr"]
    volumes: [ "${WS}:/workspace" ]
  sibling:
    image: debian:stable-slim
    depends_on:
      vivado:
        condition: service_healthy
    user: "$(id -u):$(id -g)"
    volumes: [ "${WS}:/workspace" ]
    working_dir: /workspace
    command: ["/workspace/.dv/bin/dv", "exec", "get_projects"]
EOF
    # --exit-code-from implies abort-on-container-exit in compose v2
    docker compose up --exit-code-from sibling
    check "sidecar exec via .dv/bin/dv after service_healthy" 0 $?
    docker compose run --rm sibling /workspace/.dv/bin/dv stop
    check "graceful stop from sibling" 0 $?
    docker compose down -t 30
    echo; echo "sidecar smoke: ${PASS} ok, ${FAIL} failed"
    exit $((FAIL > 0))
fi

# --- container mode ----------------------------------------------------
DV="${REPO}/scripts/dv"
export DV_WORKSPACE="${WS}"

"${DV}" start --project smoke.xpr
check "start + readiness" 0 $?

PUB="${WS}/.dv/bin/dv"
[[ -x "${PUB}" ]]; check "self-published binary" 0 $?

"${PUB}" exec 'get_projects' >/dev/null;           check "exec success" 0 $?
"${PUB}" exec 'this_is_not_tcl' >/dev/null 2>&1;   check "exec TCL error" 1 $? || true
set +e
"${PUB}" exec 'after 20000' & sleep 1
"${PUB}" exec 'puts hi' >/dev/null 2>&1;           check "busy rejection" 1 $?
wait
"${PUB}" exec --timeout 2 'after 15000' >/dev/null 2>&1; check "client timeout" 3 $?
sleep 15
"${PUB}" show result >/dev/null;                   check "result after timeout" 0 $?
"${PUB}" show status --json >/dev/null;            check "show status --json" 0 $?
"${PUB}" logs --tail 20 >/dev/null;                check "logs tail" 0 $?
"${PUB}" diagnose inspect --json >/dev/null;       check "diagnose inspect" 0 $?
"${PUB}" run bootgen -image /nonexistent.bif >/dev/null 2>&1
[[ $? -ne 0 ]];                                    check "run bootgen exit passthrough" 0 $?
"${PUB}" stop;                                     check "graceful stop" 0 $?
set -e

owner="$(stat -c %u:%g "${WS}/.dv/metadata.json")"
[[ "${owner}" == "$(id -u):$(id -g)" ]]; check "host file ownership" 0 $?

echo; echo "container smoke: ${PASS} ok, ${FAIL} failed"
exit $((FAIL > 0))
