#!/usr/bin/env bash
#
# autodeploy.sh — poll origin/master and redeploy rubc.app on a new commit.
#
# Vercel-style continuous deployment for the rubc.app VM. Run periodically by
# rubc-autodeploy.timer (see deploy/systemd/). On each run it fetches the remote,
# compares the deployed SHA against origin/master, and — only when they differ —
# pulls, rebuilds the production image, and restarts the stack.
#
# Idempotent and safe: no remote change => no-op (cheap `git fetch` only). A build
# failure leaves the currently-running container untouched (compose only swaps the
# container after a successful build).
#
# Install: see deploy/systemd/README.md. Logs go to journald (journalctl -u
# rubc-autodeploy) and to $LOG_FILE.

set -euo pipefail

REPO_DIR="${RUBC_REPO_DIR:-/root/rubc}"
BRANCH="${RUBC_BRANCH:-master}"
COMPOSE_FILE="${RUBC_COMPOSE_FILE:-docker-compose.prod.yml}"
LOG_FILE="${RUBC_LOG_FILE:-/var/log/rubc-autodeploy.log}"
HEALTH_URL="${RUBC_HEALTH_URL:-https://localhost/}"

log() {
  printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*" | tee -a "$LOG_FILE"
}

cd "$REPO_DIR"

# Fetch only the tracked branch; quiet unless something is wrong.
if ! git fetch --quiet origin "$BRANCH"; then
  log "ERROR git fetch failed; skipping this cycle"
  exit 1
fi

LOCAL_SHA="$(git rev-parse HEAD)"
REMOTE_SHA="$(git rev-parse "origin/${BRANCH}")"

if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
  # Up to date — the common case. Stay silent in the log to avoid noise.
  exit 0
fi

log "NEW COMMIT detected on origin/${BRANCH}: ${LOCAL_SHA:0:12} -> ${REMOTE_SHA:0:12}"

# Fast-forward only. If the remote was force-pushed (non-ff), do NOT blindly reset
# production — flag it and bail so a human can decide.
if ! git merge-base --is-ancestor "$LOCAL_SHA" "$REMOTE_SHA"; then
  log "ERROR origin/${BRANCH} is not a fast-forward of the deployed commit"
  log "ERROR (likely a force-push/history rewrite) — refusing to auto-deploy; resolve manually"
  exit 1
fi

log "pulling…"
git merge --ff-only "origin/${BRANCH}" >>"$LOG_FILE" 2>&1

log "building production image…"
if ! docker compose -f "$COMPOSE_FILE" build >>"$LOG_FILE" 2>&1; then
  log "ERROR build failed; current container left running, working tree at ${REMOTE_SHA:0:12}"
  exit 1
fi

log "restarting stack…"
docker compose -f "$COMPOSE_FILE" up -d >>"$LOG_FILE" 2>&1

# Best-effort health check (origin serves the CF Origin cert, so -k is expected).
sleep 3
CODE="$(curl -sk -o /dev/null -w '%{http_code}' --max-time 15 "$HEALTH_URL" || echo 000)"
if [ "$CODE" = "200" ]; then
  log "DEPLOYED ${REMOTE_SHA:0:12} — health ${CODE} OK"
else
  log "WARN deployed ${REMOTE_SHA:0:12} but health check returned ${CODE} (check the container)"
fi

# Reclaim space from the previous image build.
docker image prune -f >>"$LOG_FILE" 2>&1 || true
