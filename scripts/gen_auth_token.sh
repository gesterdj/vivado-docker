#!/bin/bash
# Generates the AMD/Xilinx installer auth token used by the base image
# build. Runs the downloaded slim (web) installer's AuthTokenGen batch
# mode, which prompts for your AMD account email and password, then
# writes the token to ~/.Xilinx/wi_authentication_key.
#
# Usage:
#   ./scripts/gen_auth_token.sh <path-to-slim-installer.bin>
#
# The token is consumed by `make build-base` as a BuildKit secret; your
# credentials never enter the Docker build.

set -euo pipefail

TOKEN_FILE="${HOME}/.Xilinx/wi_authentication_key"

INSTALLER="${1:-}"
if [[ -z "${INSTALLER}" ]]; then
	echo "Usage: $0 <path-to-slim-installer.bin>" >&2
	echo "Download the 'Web Installer' .bin from https://www.xilinx.com/support/download.html" >&2
	exit 1
fi

if [[ ! -f "${INSTALLER}" ]]; then
	echo "Error: installer '${INSTALLER}' does not exist." >&2
	exit 1
fi

if [[ ! -x "${INSTALLER}" ]]; then
	chmod +x "${INSTALLER}"
fi

echo "Running AMD installer AuthTokenGen (you will be asked to log in)..."
"${INSTALLER}" -- -b AuthTokenGen

if [[ ! -f "${TOKEN_FILE}" ]]; then
	echo "Error: token file '${TOKEN_FILE}' was not created." >&2
	echo "Check the installer output above for authentication errors." >&2
	exit 1
fi

echo "Auth token written to ${TOKEN_FILE}"
echo "Note: tokens expire; re-run this script if a build fails to authenticate."
