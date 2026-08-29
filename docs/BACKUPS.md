# Backups

Nightly at **03:30** (systemd `accountir-backup.timer` → `accountir-backup.service`
→ `/usr/local/bin/accountir-backup.sh`; canonical sources in `ops/backup/`).

## What is backed up

| Item | Source | Why |
|---|---|---|
| `accountir_cloud.dump` | `pg_dump -Fc accountir_cloud` | The SaaS system of record: all companies' books, users, invoices, tax forms, Plaid items |
| `accountir.db` | `VACUUM INTO` copy of `/home/ubuntu/accountir-data/accountir.db` | Local sync-server event ledger |
| `etc-config.tgz` | `/etc/accountir-cloud/env`, `/etc/accountir-agentd/env`, oauth2-proxy cfg | Secrets — **`PLAID_TOKEN_ENC_KEY` and `SESSION_COOKIE_KEY` are unrecoverable if lost** (every bank link dies with the key) |

## Where

- **S3:** `s3://accountir-backups-458221229419/nightly/<timestamp>/` — private
  bucket, public access blocked, SSE-S3 at rest, AWS account 458221229419
  (IAM user `web-manager`, credentials: profile `webmgr` in `/home/ubuntu/.aws/`).
- **Local staging:** `/home/ubuntu/backups/accountir/<timestamp>/` (7-day retention).
- **S3 retention:** 90 days, pruned by the script itself (the IAM user cannot set
  bucket lifecycle rules; if you get an admin credential, replace the prune loop
  with a real lifecycle rule).

## Operate

```bash
systemctl list-timers accountir-backup.timer   # next run
sudo systemctl start accountir-backup.service  # run one now
journalctl -u accountir-backup -n 20           # last run's log
aws s3 ls s3://accountir-backups-458221229419/nightly/ --profile webmgr
```

## Restore (tested 2026-08-29 — verify quarterly)

```bash
# 1. Fetch a snapshot
aws s3 cp --recursive s3://accountir-backups-458221229419/nightly/<STAMP>/ /tmp/restore/ --profile webmgr

# 2. Postgres (on the target host; stop accountir-cloud + agentd first)
sudo -u postgres createdb accountir_cloud_restore
sudo -u postgres pg_restore -d accountir_cloud_restore /tmp/restore/accountir_cloud.dump
# then either rename databases or point DATABASE_URL at the restore

# 3. SQLite
cp /tmp/restore/accountir.db /home/ubuntu/accountir-data/accountir.db

# 4. Secrets/config
sudo tar xzf /tmp/restore/etc-config.tgz -C /
sudo systemctl restart accountir accountir-cloud accountir-agentd
```

Full new-host migration checklist: `HANDOFF.md` §8.
