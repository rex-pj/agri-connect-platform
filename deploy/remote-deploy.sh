#!/usr/bin/env bash
# Runs on the VM, invoked by .github/workflows/deploy.yml over IAP SSH:
#
#   DOMAIN=... GCP_PROJECT_ID=... REGISTRY_HOST=... \
#   bash remote-deploy.sh <web-image> <legacy-image>
#
# Renders env files from Secret Manager, starts the new images, waits for
# /health/ready, and puts the previous images back if they never get ready.
set -euo pipefail

WEB_IMAGE="$1"
LEGACY_IMAGE="$2"
: "${DOMAIN:?DOMAIN is required}"
: "${GCP_PROJECT_ID:?GCP_PROJECT_ID is required}"
: "${REGISTRY_HOST:?REGISTRY_HOST is required (e.g. asia-southeast1-docker.pkg.dev)}"

APP_DIR=/opt/agri-connect
ETC_DIR=/etc/agri-connect
SRC_DIR="$(cd "$(dirname "$0")" && pwd)"

secret() {
  gcloud secrets versions access latest --secret="$1" --project="$GCP_PROJECT_ID"
}
optional_secret() {
  secret "$1" 2>/dev/null || true
}

# ── Files ─────────────────────────────────────────────────────────────────────
sudo install -d -m 755 "$APP_DIR" "$APP_DIR/db-init"
sudo install -m 644 "$SRC_DIR/compose.yaml" "$SRC_DIR/Caddyfile" "$APP_DIR/"
sudo install -m 755 "$SRC_DIR/db-init/01-app-role.sh" "$APP_DIR/db-init/"
sudo install -d -m 700 "$ETC_DIR"

# Backups: installed every deploy so a changed script ships with the release.
# The timer is enabled only once backup.env names a bucket (README, step 7).
sudo install -d -m 755 "$APP_DIR/backup"
sudo install -m 755 "$SRC_DIR/backup/agri-backup.sh" "$APP_DIR/backup/"
sudo install -m 644 "$SRC_DIR/backup/agri-backup.service" "$SRC_DIR/backup/agri-backup.timer" /etc/systemd/system/
sudo systemctl daemon-reload
if sudo test -f "$ETC_DIR/backup.env"; then
  sudo systemctl enable --now agri-backup.timer >/dev/null
else
  echo "WARNING: $ETC_DIR/backup.env missing; nightly backups are NOT running" >&2
fi

# ── Secrets → env files (mode 600, never in the image or the repo) ────────────
umask 077
tmp="$(mktemp)"
{
  echo "JWT_SECRET=$(secret agri-jwt-secret)"
  smtp_host="$(optional_secret agri-smtp-host)"
  if [ -n "$smtp_host" ]; then
    echo "SMTP_HOST=$smtp_host"
    echo "SMTP_PORT=$(optional_secret agri-smtp-port)"
    echo "SMTP_USERNAME=$(optional_secret agri-smtp-username)"
    echo "SMTP_PASSWORD=$(optional_secret agri-smtp-password)"
    echo "FROM_EMAIL=$(optional_secret agri-from-email)"
  fi
} > "$tmp"
sudo install -m 600 "$tmp" "$ETC_DIR/web.env"

compose_env() {
  cat <<ENV
DOMAIN=$DOMAIN
WEB_IMAGE=$1
LEGACY_IMAGE=$2
POSTGRES_PASSWORD=$(secret agri-postgres-password)
APP_DB_PASSWORD=$(secret agri-app-db-password)
ENV
}

previous=""
if sudo test -f "$ETC_DIR/compose.env"; then
  previous="$(sudo cat "$ETC_DIR/compose.env")"
fi
compose_env "$WEB_IMAGE" "$LEGACY_IMAGE" > "$tmp"
sudo install -m 600 "$tmp" "$ETC_DIR/compose.env"
rm -f "$tmp"

compose() {
  sudo docker compose --project-directory "$APP_DIR" -f "$APP_DIR/compose.yaml" \
    --env-file "$ETC_DIR/compose.env" "$@"
}

wait_ready() {
  for _ in $(seq 1 30); do
    if compose exec -T web /app/agri-connect healthcheck ready >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  return 1
}

# ── Roll out ──────────────────────────────────────────────────────────────────
gcloud auth configure-docker "$REGISTRY_HOST" --quiet >/dev/null
sudo gcloud auth configure-docker "$REGISTRY_HOST" --quiet >/dev/null
compose pull --quiet
compose up -d --remove-orphans

if wait_ready; then
  echo "deployed $WEB_IMAGE"
  sudo docker image prune -f >/dev/null
  exit 0
fi

echo "new release never became ready; last web logs:" >&2
compose logs --tail 50 web >&2 || true

if [ -n "$previous" ]; then
  echo "rolling back to the previous images" >&2
  printf '%s\n' "$previous" | sudo tee "$ETC_DIR/compose.env" >/dev/null
  compose up -d --remove-orphans
  wait_ready || echo "rollback is not ready either; investigate on the VM" >&2
fi
exit 1
