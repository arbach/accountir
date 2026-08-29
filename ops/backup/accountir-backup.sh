#!/usr/bin/env bash
# Nightly backup of everything accountir needs to survive a dead host:
#   1. PostgreSQL accountir_cloud (system of record: books, users, tax, Plaid items)
#   2. SQLite sync-server ledger (consistent snapshot via VACUUM INTO)
#   3. Service config/secrets (/etc env files; PLAID_TOKEN_ENC_KEY is load-bearing)
# Staged locally (7-day retention), then uploaded to a private S3 bucket
# (SSE-S3 at rest, public access blocked; script-side 90-day retention because
# the IAM user cannot set bucket lifecycle rules).
#
# Installed as: /usr/local/bin/accountir-backup.sh (run by accountir-backup.timer, 03:30 daily)
# Restore procedure: see docs/BACKUPS.md in the repo.
set -euo pipefail

BUCKET="s3://accountir-backups-458221229419"
STAGE="/home/ubuntu/backups/accountir"
STAMP="$(date +%Y%m%d-%H%M%S)"
DIR="$STAGE/$STAMP"
export AWS_SHARED_CREDENTIALS_FILE=/home/ubuntu/.aws/credentials
export AWS_CONFIG_FILE=/home/ubuntu/.aws/config
export AWS_PROFILE=webmgr

mkdir -p "$DIR"
# The SQLite snapshot runs as user ubuntu (the db owner) — it must be able to
# write into the staging dir even though the service runs as root.
chown ubuntu:ubuntu "$DIR"

# 1. Postgres — custom format (compressed, pg_restore-able).
runuser -u postgres -- pg_dump -Fc accountir_cloud > "$DIR/accountir_cloud.dump"

# 2. SQLite — VACUUM INTO produces a consistent copy even mid-write.
rm -f "$DIR/accountir.db"
runuser -u ubuntu -- sqlite3 /home/ubuntu/accountir-data/accountir.db "VACUUM INTO '$DIR/accountir.db'"

# 3. Config/secrets (root-readable only).
tar czf "$DIR/etc-config.tgz" \
  /etc/accountir-cloud/env \
  /etc/accountir-agentd/env \
  /etc/oauth2-proxy/oauth2-proxy.cfg 2>/dev/null
chmod 600 "$DIR"/*

# Upload.
aws s3 cp --recursive --no-progress "$DIR" "$BUCKET/nightly/$STAMP/"

# S3 retention: delete nightly/ prefixes older than 90 days (best-effort).
CUTOFF="$(date -d '90 days ago' +%Y%m%d)"
aws s3 ls "$BUCKET/nightly/" | awk '{print $2}' | tr -d '/' | while read -r p; do
  [ -n "$p" ] && [ "${p%%-*}" -lt "$CUTOFF" ] 2>/dev/null && \
    aws s3 rm --recursive --quiet "$BUCKET/nightly/$p/" || true
done

# Local retention: 7 days.
find "$STAGE" -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf {} +

echo "backup $STAMP OK: $(du -sh "$DIR" | cut -f1) uploaded to $BUCKET/nightly/$STAMP/"
