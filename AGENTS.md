# AGENTS.md

This repo builds Docker images for AMD FPGA tools (Vivado, Vitis).
`README.md` is the single source of truth for build, run, and environment
instructions — read it before making changes.

## Guidelines

- Use [Conventional Commits](https://www.conventionalcommits.org/) for
  commit messages (`feat:`, `fix:`, `docs:`, `chore:`)
- Format markdown to 80-column layout

## Repository layout

See [README → Repository layout](README.md#repository-layout).
Scripts go in `scripts/`, installer configs in `config/`, container build
sources in `docker/`, the daVit session CLI crate in `davit/`,
supplementary docs in `docs/`.

## Instructions

- Build: [README → Maintenance](README.md#maintenance)
- Run (GUI/batch): [README → Running Vivado from the
  Image](README.md#running-vivado-from-the-image)
- Environment variables: see the `run.vivado.sh` table in the same section
- Persistent sessions / CLI: [README → daVit: Persistent Session
  CLI](README.md#davit-persistent-session-cli)

## Constraints

- Free-to-use Vitis Unified tool/device selection only (no paid editions)
- Headless build environment
- No public image hosting (size and licensing)
- Installer must be downloaded from AMD separately
