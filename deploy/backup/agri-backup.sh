#!/usr/bin/env bash
# Nightly logical backup of the database to GCS (agri-backup.timer).
# The bucket has a 30-day lifecycle rule and object versioning (deploy/README.md).
set -euo pipefail

# BACKUP_BUCKET=<bucket name, no gs://>
# shellcheck source=/dev/null  # lives on the VM, not in the repo
source /etc/agri-connect/backup.env

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
object="gs://${BACKUP_BUCKET}/postgres/agri_connect-${stamp}.dump"

# pipefail makes a failed pg_dump fail the unit instead of uploading a
# truncated file that looks like a good backup.
# spatial_ref_sys rows ship with the PostGIS extension; dumping them makes the
# restore fail, because only the superuser may write that table (rehearsed).
docker compose --project-directory /opt/agri-connect -f /opt/agri-connect/compose.yaml \
  --env-file /etc/agri-connect/compose.env \
  exec -T db pg_dump -U postgres -Fc --exclude-table-data=public.spatial_ref_sys agri_connect \
  | gcloud storage cp - "$object"

echo "backup written to $object"
