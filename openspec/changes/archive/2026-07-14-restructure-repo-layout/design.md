## Context

The repo root currently mixes concerns: two installer config files
(`install_config.txt`, `xsetup_config_25.txt`), two run scripts (`run.sh`,
`run.vivado.sh`), a 35KB spec document (`fpgatools-docker-spec.md`), the
`Makefile`, `README.md`, `AGENTS.md`, and a `docker/` folder. `README.md`
and `AGENTS.md` both carry build/run/environment instructions that have
already drifted (AGENTS has env-var and Apple Silicon details missing from
its README pointers, README has troubleshooting AGENTS lacks).

Constraints:
- `Makefile` build.stamp target depends on `install_config.txt` and the
  Dockerfile COPYs it into the build context.
- Docker build context is the repo root; `.dockerignore` currently minimal.
- Users invoke `./run.vivado.sh` directly per README/AGENTS; changing its
  path is user-visible.

## Goals / Non-Goals

**Goals:**
- One folder per file kind: `scripts/`, `config/`, `docker/`, `docs/`.
- README.md is the single authoritative usage document.
- AGENTS.md contains only agent conventions plus links into README.
- All build tooling (Makefile, Dockerfile, .dockerignore) works after moves.

**Non-Goals:**
- No changes to Docker image content, install flow, or Vivado versions.
- No renaming of the repo, image tags, or Make targets.
- No backwards-compat symlinks (repo is small; a clean break is fine).

## Decisions

1. **Folder names: `scripts/`, `config/`, `docs/`** — conventional,
   self-describing names. Alternative considered: `bin/` for scripts
   (rejected: these are not installed binaries), `etc/` for config
   (rejected: obscure for newcomers).

2. **Delete `run.sh`** — it is an empty placeholder ("For now, empty.")
   with no callers. Alternative: move it to `scripts/` (rejected: dead
   weight).

3. **Keep `Makefile`, `README.md`, `AGENTS.md`, `LICENSE` at root** —
   standard tooling expectation (make, GitHub rendering, agent discovery).

4. **README owns all instructions; AGENTS.md references sections** —
   AGENTS.md keeps only: commit conventions, markdown formatting rule,
   constraints summary, and links like "see README → Build". This removes
   the dual-maintenance drift. Alternative: keep full duplication in
   AGENTS.md (rejected: it is what caused the current drift).

5. **Use `git mv` for all moves** — preserves history.

6. **Makefile forwards paths** — update prerequisites to
   `config/install_config.txt` and pass its path as a build-arg or keep the
   Dockerfile COPY path in sync (`COPY config/install_config.txt ...`).

## Risks / Trade-offs

- [Users' scripts call `./run.vivado.sh`] → Breaking change is documented
  in README changelog note and commit message; path is short and easy to
  update.
- [Dockerfile COPY misses new path → build fails late] → Verify with a
  Dockerfile syntax/path check (grep) since a full 200GB build is
  impractical; the Makefile guard already fails fast on missing files.
- [xsetup_config_25.txt purpose unclear] → Move as-is into `config/`;
  do not delete without owner confirmation.

## Migration Plan

1. `git mv` files into new folders; delete `run.sh`.
2. Update Makefile, Dockerfile, .dockerignore references.
3. Rewrite README (add Repository layout section, absorb any AGENTS-only
   content such as env var table); slim AGENTS.md.
4. Single commit (`refactor: restructure repo layout and consolidate docs`);
   rollback = revert the commit.

## Open Questions

- Should `install_config.txt` be version-suffixed like
  `xsetup_config_25.txt`, or should the two configs be merged/renamed for
  clarity? (Default: move as-is.)
