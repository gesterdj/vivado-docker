## 1. Move Files

- [x] 1.1 `git mv run.vivado.sh scripts/run.vivado.sh` (create `scripts/`)
- [x] 1.2 `git mv install_config.txt xsetup_config_25.txt` into `config/`
- [x] 1.3 `git mv fpgatools-docker-spec.md docs/fpgatools-docker-spec.md`
- [x] 1.4 `git rm run.sh` (empty placeholder, no callers)

## 2. Update Build Tooling

- [x] 2.1 Makefile: change `install_config.txt` prerequisite to
      `config/install_config.txt` in `build.stamp` and the docker-save
      target
- [x] 2.2 docker/Dockerfile: update COPY path for `install_config.txt` to
      `config/install_config.txt`
- [x] 2.3 Review `.dockerignore`/`.gitignore` and update paths for moved
      files; ensure `config/` is included in the build context
- [x] 2.4 Verify: `make` (info target) runs; grep repo for old paths
      (`./run.vivado.sh`, root `install_config.txt`, `run.sh`) — no build
      file references remain

## 3. Consolidate Documentation

- [x] 3.1 README.md: add "Repository layout" section describing `scripts/`,
      `config/`, `docker/`, `docs/`
- [x] 3.2 README.md: absorb AGENTS-only content (environment variable list,
      CLI/GUI mode notes, batch example) and update all script/config paths
      to new locations; keep 80-column formatting
- [x] 3.3 AGENTS.md: slim to conventions (Conventional Commits, 80-column
      markdown), constraints, and links to README sections for build/run/
      environment
- [x] 3.4 Verify: no build/run/env instructions duplicated between README
      and AGENTS; all links resolve

## 4. Finalize

- [x] 4.1 `chmod +x scripts/run.vivado.sh` check (executable bit preserved
      by git mv)
- [x] 4.2 Commit as `refactor: restructure repo layout and consolidate
      docs` with Co-authored-by trailer
