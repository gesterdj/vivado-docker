## Why

The repository root is cluttered: install configs (`install_config.txt`,
`xsetup_config_25.txt`), run scripts (`run.sh`, `run.vivado.sh`), and a large
spec document all sit at the top level, and instructions are duplicated and
drifting between `README.md` and `AGENTS.md`. A clear folder structure and a
single source of truth for usage docs makes the repo easier to navigate and
maintain.

## What Changes

- **BREAKING** Move run scripts into `scripts/` (`scripts/run.vivado.sh`);
  remove the empty placeholder `run.sh`.
- **BREAKING** Move installer configuration files into `config/`
  (`config/install_config.txt`, `config/xsetup_config_25.txt`).
- Keep `docker/` as-is for `Dockerfile` and `udev_stub.c`.
- Move `fpgatools-docker-spec.md` into `docs/`.
- Update `Makefile`, `docker/Dockerfile`, and `.dockerignore` to reference the
  new paths.
- Consolidate documentation: `README.md` becomes the single source of truth
  for build/run/usage instructions; `AGENTS.md` is slimmed to agent-specific
  guidance (conventions, constraints) plus pointers into README sections.
- Update README with a "Repository layout" section describing the structure.

## Capabilities

### New Capabilities
- `repo-layout`: Defines the canonical folder structure (scripts/, config/,
  docker/, docs/) and where new files of each kind belong.
- `docs-structure`: Defines the division of content between README.md
  (authoritative user instructions) and AGENTS.md (agent conventions that
  reference, not duplicate, the README).

### Modified Capabilities

<!-- none — existing specs (prompts, skills) are unaffected -->

## Impact

- `Makefile`: paths to `install_config.txt` and script targets.
- `docker/Dockerfile`: COPY paths for install config.
- `run.vivado.sh` → `scripts/run.vivado.sh`; any docs referencing `./run.vivado.sh`.
- `.dockerignore` / `.gitignore`: path updates.
- `README.md` and `AGENTS.md`: content reorganization; users with muscle
  memory for old paths (breaking for existing local workflows).
