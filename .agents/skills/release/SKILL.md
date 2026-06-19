---
name: release
description: Use when cutting, publishing, or shipping a new release of rubc — when the user asks to "release", "cut a release", "ship a version", "publish", "tag a release", "bump the version", "make release notes", or "create a GitHub release". Drives scripts/release.sh + .github/workflows/release.yml to auto-bump the version from conventional commits, author release notes, tag/push, and attach cross-platform binaries.
---

# Releasing rubc

Cut a versioned rubc release: auto-bump the version from conventional commits,
author human-quality release notes, tag + push, and attach cross-platform
binaries — using the repo's own tooling. Never hand-roll a release; always go
through `just release`.

## Architecture (three roles)

- **`scripts/release.sh`** (run via `just release`) — the deterministic engine:
  computes the next version, bumps every crate + `Cargo.lock`, runs `just check`,
  commits, annotated-tags (with the notes), pushes, and creates the GitHub release.
- **You (the agent)** — author the release notes at run time from the dry-run output.
- **`.github/workflows/release.yml`** — on the pushed `vX.Y.Z` tag, builds native
  macOS (arm64 + x64) / Linux / Windows binaries and uploads them to the release.

## Versioning

Standard semver from conventional commits since the last `vX.Y.Z` tag:
`feat:` → minor, `fix:` → patch, `feat!:` / `fix!:` / `BREAKING CHANGE` → major.
The script computes it; override with `--version X.Y.Z` only when you must.

## The flow

### 1. Preview (mutates nothing)

```bash
just release --dry-run
```

Read the output: the **computed version** and every commit since the last tag,
grouped into Features / Fixes / Other / Misc. This is your source material for
the notes. Stop and resolve anything surprising (a `feat` that should have been a
`fix`, an unexpected major bump, a missing change).

### 2. Author the release notes

Write user-facing notes to a temp file — do NOT just dump commit subjects.

```bash
NOTES="$(mktemp -t rubc-notes.XXXXXX.md)"
# ...write markdown into "$NOTES"...
```

Good rubc notes:
- Open with a 1–2 sentence summary of what this release delivers.
- Group under `## Highlights`, `## Features`, `## Fixes`, `## Web / PWA`.
- Translate terse subjects into value — e.g. `feat(web): toggleable WebGL
  post-processing shaders — LCD-grid/scanline/CRT` becomes "CRT / LCD-grid /
  scanline display shaders in the web player (toggle in settings)".
- For an emulator, lead with accuracy / compatibility wins (test-ROM passes, new
  MBCs, CGB behaviour, specific games fixed) — that is what users care about.
- Strip internal bead IDs (`rubc-xxxx`) and noisy scopes; keep the meaning.
- End with the `**Full Changelog**: <repo>/compare/<prev>...v<next>` line (the
  dry-run prints it — reuse it).
- Omit pure chore/test/docs churn unless it matters to users.

### 3. Confirm with the user (REQUIRED — public + irreversible)

Show the user the computed version and the drafted notes. Get explicit go-ahead
before cutting. Pushing the tag publishes the release and triggers the binary
builds — there is no quiet undo.

### 4. Cut the release

```bash
just release --notes-file "$NOTES" --yes
# override the computed version only if needed:
# just release --notes-file "$NOTES" --version 1.0.0 --yes
```

Preflight runs first (on master, clean tree, in sync with origin, `just check`).
If `just check` fails, the version bump is reverted and nothing is committed.

### 5. Watch CI attach the binaries, then verify

```bash
gh run watch "$(gh run list --workflow=release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
gh release view "vX.Y.Z"   # confirm the notes render + all 4 assets are attached
```

The release + notes are live immediately; the four
`rubc-vX.Y.Z-<target>.tar.gz|.zip` assets stream in as each matrix job finishes.

## Troubleshooting

- **`just check` failed** — fix it, commit, then re-run the release. Nothing was
  tagged or pushed.
- **A platform binary failed in CI** — fix the cause, then re-run only the builds:
  Actions → release → Run workflow → enter the tag (`workflow_dispatch`). The
  release + notes stay intact; uploads use `--clobber`.
- **Wrong version computed** — re-run with `--version X.Y.Z`.
- **Notes need editing after publish** — `gh release edit vX.Y.Z --notes-file NEW.md`.
- **`gh release create` failed but the tag pushed** — the CI `ensure-release` job
  creates the release from the annotated tag's notes; or run
  `gh release create vX.Y.Z --notes-file "$NOTES" --verify-tag` manually.

## Never

- Never push a `vX.Y.Z` tag without explicit user confirmation — it is public.
- Never hand-edit crate versions or create the tag yourself to release; always
  go through `just release`.
- Never weaken, skip, or `--no-verify` past `just check` to force a release
  (`--skip-check` is only valid when you JUST ran the full gate yourself).

## Flags (`scripts/release.sh`)

| Flag | Effect |
|------|--------|
| `--dry-run` | Preview version + categorized commits; mutate nothing. |
| `--notes-file PATH` | Your authored notes (falls back to an auto changelog). |
| `--version X.Y.Z` | Override the computed bump. |
| `--yes` | Execute non-interactively (required when stdin is not a TTY). |
| `--skip-check` | Skip `just check` (only if you just ran it). |
