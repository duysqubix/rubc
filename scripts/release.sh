#!/usr/bin/env bash
#
# release.sh — cut a new rubc release.
#
# Three roles, kept deliberately separate:
#
#   * THIS SCRIPT owns the deterministic mechanics: compute the next version from
#     conventional commits since the last vX.Y.Z tag (standard semver —
#     feat->minor, fix->patch, '!'/'BREAKING CHANGE'->major), bump every crate
#     version + Cargo.lock, verify (`just check`), commit, tag, push, and create
#     the GitHub release.
#
#   * THE OPERATOR (a human, or the AI agent driving this) authors the release
#     NOTES and passes them with --notes-file. `--dry-run` prints the computed
#     version and the categorized commit list so notes can be written from a
#     clean source. With no --notes-file an auto-generated grouped changelog is
#     used as a fallback.
#
#   * CI (.github/workflows/release.yml), triggered by the pushed tag, builds the
#     macOS / Linux / Windows binaries and uploads them to this release.
#
# Usage:
#   scripts/release.sh --dry-run                      # preview, mutate nothing
#   scripts/release.sh --notes-file NOTES.md --yes    # cut the release
#   scripts/release.sh --version 1.0.0 --yes          # override computed version
#
# Flags:
#   --dry-run          Print the plan (next version + categorized commits); no mutation.
#   --version X.Y.Z    Use this exact version instead of the computed bump.
#   --notes-file PATH  Markdown release notes. Defaults to an auto-generated changelog.
#   --yes              Execute non-interactively (required to mutate when stdin is not a TTY).
#   --skip-check       Skip `just check` (use only if you just ran it).
#   -h, --help         Show this help and exit.

set -euo pipefail

# --- config -----------------------------------------------------------------
CRATE_MANIFESTS=(rubc/Cargo.toml rubc-ng/Cargo.toml rubc-wasm/Cargo.toml)
TAG_GLOB='v[0-9]*'

# --- helpers ----------------------------------------------------------------
die()  { printf 'release: %s\n' "$*" >&2; exit 1; }
note() { printf '\033[1m%s\033[0m\n' "$*"; }

usage() { sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; $d'; }

# Normalise the origin remote (git@ or https) to an https://github.com/owner/repo URL.
repo_url() {
  local url; url="$(git remote get-url origin)"
  url="${url%.git}"
  url="${url/git@github.com:/https://github.com/}"
  url="${url/https:\/\/github.com\//https://github.com/}"
  printf '%s' "$url"
}

# Increment major.minor.patch by the given bump kind.
bump_version() {
  local cur="$1" kind="$2" M m p
  IFS=. read -r M m p <<<"$cur"
  case "$kind" in
    major) printf '%d.0.0' "$((M + 1))" ;;
    minor) printf '%d.%d.0' "$M" "$((m + 1))" ;;
    patch) printf '%d.%d.%d' "$M" "$m" "$((p + 1))" ;;
    *) die "internal: unknown bump kind '$kind'" ;;
  esac
}

# Classify the bump for a commit range using conventional-commit rules.
# Echoes one of: major | minor | patch | none
classify_bump() {
  local range="$1" subjects bodies
  subjects="$(git log "$range" --pretty=%s)"
  bodies="$(git log "$range" --pretty=%b)"
  if printf '%s\n' "$subjects" | grep -qE '^[a-z]+(\([^)]+\))?!:' \
     || printf '%s\n' "$bodies" | grep -qE '^BREAKING[ -]CHANGE:'; then
    printf 'major'
  elif printf '%s\n' "$subjects" | grep -qE '^feat(\([^)]+\))?!?:'; then
    printf 'minor'
  elif printf '%s\n' "$subjects" | grep -qE '^fix(\([^)]+\))?!?:'; then
    printf 'patch'
  elif [ -n "$(git log "$range" --pretty=%s)" ]; then
    printf 'patch'   # commits exist but none are feat/fix/breaking (chore/docs/...)
  else
    printf 'none'
  fi
}

# Markdown, grouped by type, for the given range. Used for --dry-run and as the
# fallback release body when no --notes-file is supplied.
changelog_md() {
  local range="$1" prev_tag="$2" new_ver="$3" log rows
  log="$(git log "$range" --pretty='- %s (%h)')"
  printf '## What'\''s Changed\n\n'
  emit() {  # $1 = subject-prefix ERE, $2 = section title
    rows="$(printf '%s\n' "$log" | grep -E "^- $1" || true)"
    [ -n "$rows" ] || return 0
    printf '### %s\n%s\n\n' "$2" "$rows"
  }
  emit 'feat(\([^)]+\))?!?:' 'Features'
  emit 'fix(\([^)]+\))?!?:' 'Fixes'
  emit 'perf(\([^)]+\))?!?:' 'Performance'
  emit '(refactor|docs|build|ci|test|chore)(\([^)]+\))?!?:' 'Other'
  rows="$(printf '%s\n' "$log" | grep -vE '^- [a-z]+(\([^)]+\))?!?:' || true)"
  [ -n "$rows" ] && printf '### Misc\n%s\n\n' "$rows"
  printf '**Full Changelog**: %s/compare/%s...v%s\n' "$(repo_url)" "$prev_tag" "$new_ver"
}

# --- arg parsing ------------------------------------------------------------
DRY_RUN=0; ASSUME_YES=0; SKIP_CHECK=0; FORCE_VERSION=""; NOTES_FILE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    --skip-check) SKIP_CHECK=1 ;;
    --version) FORCE_VERSION="${2:-}"; shift ;;
    --version=*) FORCE_VERSION="${1#*=}" ;;
    --notes-file) NOTES_FILE="${2:-}"; shift ;;
    --notes-file=*) NOTES_FILE="${1#*=}" ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done

cd "$(git rev-parse --show-toplevel)"

# --- determine current + next version ---------------------------------------
PREV_TAG="$(git tag --list "$TAG_GLOB" --sort=-version:refname | head -1 || true)"
[ -n "$PREV_TAG" ] || die "no vX.Y.Z tag found to diff against"
PREV_VERSION="${PREV_TAG#v}"
RANGE="${PREV_TAG}..HEAD"

[ -n "$(git log "$RANGE" --oneline)" ] || die "no commits since $PREV_TAG — nothing to release"

if [ -n "$FORCE_VERSION" ]; then
  NEW_VERSION="${FORCE_VERSION#v}"
  [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "invalid --version '$FORCE_VERSION' (want X.Y.Z)"
  BUMP="(forced)"
else
  BUMP="$(classify_bump "$RANGE")"
  [ "$BUMP" != none ] || die "no releasable commits since $PREV_TAG"
  NEW_VERSION="$(bump_version "$PREV_VERSION" "$BUMP")"
fi
NEW_TAG="v${NEW_VERSION}"

git rev-parse -q --verify "refs/tags/${NEW_TAG}" >/dev/null 2>&1 \
  && die "tag ${NEW_TAG} already exists"

# Counts for the summary line.
N_FEAT="$(git log "$RANGE" --pretty=%s | grep -cE '^feat(\([^)]+\))?!?:' || true)"
N_FIX="$(git log "$RANGE" --pretty=%s | grep -cE '^fix(\([^)]+\))?!?:' || true)"
N_BREAK="$(git log "$RANGE" --pretty='%s%n%b' | grep -cE '^([a-z]+(\([^)]+\))?!:|BREAKING[ -]CHANGE:)' || true)"

note "rubc release plan"
printf '  previous : %s\n  next     : %s  (bump: %s)\n  commits  : %s since %s — %s feat, %s fix, %s breaking\n\n' \
  "$PREV_TAG" "$NEW_TAG" "$BUMP" "$(git rev-list --count "$RANGE")" "$PREV_TAG" "$N_FEAT" "$N_FIX" "$N_BREAK"

# --- dry run: print the plan and the categorized commits, then stop ---------
if [ "$DRY_RUN" = 1 ]; then
  changelog_md "$RANGE" "$PREV_TAG" "$NEW_VERSION"
  printf '\n(dry run — nothing mutated. Write notes, then re-run with --notes-file NOTES.md --yes)\n'
  exit 0
fi

# --- preflight (mutating path) ----------------------------------------------
[ "$(git branch --show-current)" = master ] || die "must be on master to release"
git diff --quiet && git diff --cached --quiet || die "working tree is dirty — commit or stash first"
note "fetching origin…"; git fetch --quiet origin master
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/master)" ] \
  || die "HEAD is not in sync with origin/master — push/pull first"

command -v gh >/dev/null 2>&1 || die "gh CLI not found (needed to create the release)"
gh auth status >/dev/null 2>&1 || die "gh is not authenticated (run: gh auth login)"

if [ "$ASSUME_YES" = 0 ]; then
  if [ -t 0 ]; then
    read -r -p "Release ${NEW_TAG} now? [y/N] " ans
    [[ "$ans" =~ ^[Yy]$ ]] || die "aborted by user"
  else
    die "refusing to release non-interactively without --yes"
  fi
fi

# Resolve notes: explicit file, or generated changelog written to a temp file.
NOTES_PATH=""
cleanup() { [ -n "${TMP_NOTES:-}" ] && rm -f "$TMP_NOTES"; }
trap cleanup EXIT
if [ -n "$NOTES_FILE" ]; then
  [ -f "$NOTES_FILE" ] || die "--notes-file '$NOTES_FILE' not found"
  NOTES_PATH="$NOTES_FILE"
else
  TMP_NOTES="$(mktemp -t rubc-release-notes.XXXXXX)"
  changelog_md "$RANGE" "$PREV_TAG" "$NEW_VERSION" >"$TMP_NOTES"
  NOTES_PATH="$TMP_NOTES"
  note "no --notes-file given; using an auto-generated changelog"
fi

# --- bump crate versions + refresh the lockfile -----------------------------
note "bumping crate versions to ${NEW_VERSION}…"
for f in "${CRATE_MANIFESTS[@]}"; do
  perl -i -pe 'if (!$seen && s/^version = "[^"]*"/version = "'"$NEW_VERSION"'"/) { $seen = 1 }' "$f"
  grep -qE "^version = \"${NEW_VERSION}\"" "$f" || die "failed to bump version in $f"
done
cargo update --workspace --quiet

# --- verify -----------------------------------------------------------------
if [ "$SKIP_CHECK" = 0 ]; then
  note "running just check (fmt + clippy + build + tests)…"
  if ! just check; then
    git checkout -- "${CRATE_MANIFESTS[@]}" Cargo.lock
    die "just check failed — version bump reverted, nothing committed"
  fi
fi

# --- commit, tag, push ------------------------------------------------------
note "committing, tagging, pushing…"
git add "${CRATE_MANIFESTS[@]}" Cargo.lock
git commit -m "chore(release): ${NEW_TAG}"
git tag -a "${NEW_TAG}" -F "$NOTES_PATH"
git push origin master
git push origin "${NEW_TAG}"

# --- create the GitHub release (CI attaches binaries on the tag push) -------
note "creating GitHub release ${NEW_TAG}…"
gh release create "${NEW_TAG}" \
  --title "${NEW_TAG}" \
  --notes-file "$NOTES_PATH" \
  --verify-tag

note "released ${NEW_TAG}"
printf '  release : %s/releases/tag/%s\n  binaries: building in CI → %s/actions\n' \
  "$(repo_url)" "${NEW_TAG}" "$(repo_url)"
