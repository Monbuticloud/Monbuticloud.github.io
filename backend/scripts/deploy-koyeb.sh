#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${KOYEB_APP_NAME:-homepage}"
REGION="${KOYEB_REGION:-fra}"
INSTANCE="${KOYEB_INSTANCE:-free}"

echo "==> Checking prerequisites..."
if ! command -v koyeb &>/dev/null; then
  echo "Error: 'koyeb' CLI not found."
  echo "Install: curl -fsSL https://cli.koyeb.com/install.sh | sh"
  exit 1
fi
if ! koyeb whoami &>/dev/null 2>&1; then
  echo "Error: Not logged in. Run 'koyeb login' first."
  exit 1
fi

# ── App service (internal, port 8000) ──
echo "==> Deploying app service (Rust backend)..."
koyeb deploy "$APP_NAME/app" . \
  --docker dockerfile \
  --dockerfile backend/Dockerfile \
  --port 8000:http \
  --region "$REGION" \
  --instance-type "$INSTANCE"

echo
# ── Nginx service (public, port 80) ──
echo "==> Deploying nginx service (reverse proxy)..."
koyeb deploy "$APP_NAME/nginx" . \
  --docker dockerfile \
  --dockerfile backend/nginx/Dockerfile.koyeb \
  --port 80:http \
  --routes /:80 \
  --region "$REGION" \
  --instance-type "$INSTANCE"

echo
echo "==> Deployed! Your app will be available at:"
echo "    https://$APP_NAME-<org-hash>.koyeb.app"
echo
echo "Run 'koyeb app logs $APP_NAME/nginx' to tail logs."
