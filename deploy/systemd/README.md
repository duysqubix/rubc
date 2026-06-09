# Auto-deploy (Vercel-style CD) for rubc.app

A small watchdog on the production VM polls `origin/master` and, when a new commit
appears, pulls it, rebuilds the production image, and restarts the stack — no manual
SSH needed. Push to `master` → it goes live within a couple of minutes.

## How it works

- **`deploy/autodeploy.sh`** — does the work: `git fetch`, compare deployed SHA vs
  `origin/master`, and *only on a change* fast-forward pull → `docker compose build`
  → `up -d` → health check. A no-change cycle is just a cheap `git fetch`.
- **`rubc-autodeploy.timer`** — fires the service every 2 minutes (and once on boot).
- **`rubc-autodeploy.service`** — a `oneshot` that runs the script.

### Safety properties

- **No-op when unchanged** — only redeploys on an actual SHA change.
- **Fast-forward only** — if `master` was force-pushed (history rewrite), the script
  refuses to deploy and logs an error rather than resetting production blindly.
- **Build-failure safe** — `docker compose build` runs *before* `up -d`, so a broken
  build leaves the currently-running container serving traffic.
- **Survives reboot** — it's a systemd timer; `Persistent=true` catches missed runs.

## Install on the VM (one time)

From the repo root on the VM (`/root/rubc`):

```bash
chmod +x deploy/autodeploy.sh
cp deploy/systemd/rubc-autodeploy.service /etc/systemd/system/
cp deploy/systemd/rubc-autodeploy.timer   /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now rubc-autodeploy.timer
```

## Operate

```bash
systemctl list-timers rubc-autodeploy.timer   # when it next runs
systemctl start rubc-autodeploy.service       # force a deploy check right now
journalctl -u rubc-autodeploy -f              # live logs
tail -f /var/log/rubc-autodeploy.log          # deploy history (commits + outcomes)
systemctl disable --now rubc-autodeploy.timer # pause auto-deploy
```

## Tuning

Poll interval lives in `rubc-autodeploy.timer` (`OnUnitActiveSec`). The script reads
these env overrides (set via a systemd drop-in if needed): `RUBC_REPO_DIR`,
`RUBC_BRANCH`, `RUBC_COMPOSE_FILE`, `RUBC_LOG_FILE`, `RUBC_HEALTH_URL`.
