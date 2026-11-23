# Deploying Agri-Connect to Google Compute Engine

One VM runs everything with docker compose behind Cloudflare's proxy: Cloudflare → Caddy (TLS with a Cloudflare Origin Certificate) → the Rust server, which serves every page and the API, and PostgreSQL + PostGIS. GitHub Actions builds the images, pushes them to Artifact Registry and rolls them out over IAP SSH. No service-account key file exists anywhere: GitHub authenticates with Workload Identity Federation, and the VM uses its attached service account.

Everything below is done once. Afterwards a merge to `develop` deploys by itself once CI passes.

Replace these placeholders throughout:

| Placeholder | Example | Notes |
|---|---|---|
| `PROJECT_ID` | `agri-connect-prod` | |
| `REGION` / `ZONE` | `asia-southeast1` / `asia-southeast1-b` | Singapore, the nearest region to the Mekong Delta |
| `DOMAIN` | `agriconnect.vn` | a Cloudflare zone; its `@` and `www` A records point at the VM's static IP, **Proxied** (step 9) |
| `BACKUP_BUCKET` | `agri-connect-prod-backups` | globally unique |

## 1. Project, APIs, registry

```bash
gcloud config set project PROJECT_ID
gcloud services enable compute.googleapis.com artifactregistry.googleapis.com \
  secretmanager.googleapis.com iamcredentials.googleapis.com iap.googleapis.com
gcloud artifacts repositories create agri-connect --repository-format=docker --location=REGION
```

## 2. Service accounts

```bash
# The VM's identity: pull images, read secrets, write backups.
gcloud iam service-accounts create agri-vm
VM_SA=agri-vm@PROJECT_ID.iam.gserviceaccount.com
gcloud projects add-iam-policy-binding PROJECT_ID --member="serviceAccount:$VM_SA" --role=roles/artifactregistry.reader
gcloud projects add-iam-policy-binding PROJECT_ID --member="serviceAccount:$VM_SA" --role=roles/secretmanager.secretAccessor

# GitHub's identity: push images, SSH to the VM through IAP.
gcloud iam service-accounts create agri-deploy
DEPLOY_SA=agri-deploy@PROJECT_ID.iam.gserviceaccount.com
for role in roles/artifactregistry.writer roles/compute.osAdminLogin roles/iap.tunnelResourceAccessor roles/compute.viewer; do
  gcloud projects add-iam-policy-binding PROJECT_ID --member="serviceAccount:$DEPLOY_SA" --role="$role"
done
gcloud iam service-accounts add-iam-policy-binding "$VM_SA" --member="serviceAccount:$DEPLOY_SA" --role=roles/iam.serviceAccountUser
```

## 3. Workload Identity Federation for GitHub

Only workflows from this repository can impersonate `agri-deploy`. In a project that already has a pool named `github` (another repository deploys from it), skip `pools create` and add only a provider with a name of its own (e.g. `agri-connect` in place of `github` in `providers create-oidc`, and in step 8): a deleted pool keeps its name for 30 days and cannot be created again.

```bash
gcloud iam workload-identity-pools create github --location=global
gcloud iam workload-identity-pools providers create-oidc github \
  --location=global --workload-identity-pool=github \
  --issuer-uri=https://token.actions.githubusercontent.com \
  --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository" \
  --attribute-condition="assertion.repository == 'rex-pj/agri-connect-platform'"
POOL=$(gcloud iam workload-identity-pools describe github --location=global --format='value(name)')
gcloud iam service-accounts add-iam-policy-binding "$DEPLOY_SA" \
  --role=roles/iam.workloadIdentityUser \
  --member="principalSet://iam.googleapis.com/$POOL/attribute.repository/rex-pj/agri-connect-platform"
```

## 4. Network, disk, VM

```bash
gcloud compute addresses create agri-ip --region=REGION
# The web ports only from Cloudflare: nobody reaches the origin around it.
# When Cloudflare adds a range, run it again as `firewall-rules update` and add
# the range to the Caddyfile too.
gcloud compute firewall-rules create agri-web --allow=tcp:80,tcp:443 --target-tags=agri-web \
  --source-ranges="$(curl -fsS https://www.cloudflare.com/ips-v4 | paste -sd, -)"
# SSH only through IAP (Google's range), never from the internet.
gcloud compute firewall-rules create agri-iap-ssh --allow=tcp:22 --source-ranges=35.235.240.0/20 --target-tags=agri-web

# The database lives on its own disk so it outlives the VM.
gcloud compute disks create agri-pgdata --size=20GB --type=pd-balanced --zone=ZONE

gcloud compute instances create agri-vm --zone=ZONE --machine-type=e2-small \
  --image-family=debian-12 --image-project=debian-cloud --boot-disk-size=20GB \
  --address=agri-ip --tags=agri-web --service-account="$VM_SA" --scopes=cloud-platform \
  --metadata=enable-oslogin=TRUE --disk=name=agri-pgdata,device-name=pgdata,mode=rw,auto-delete=no

# Weekly snapshots of the data disk, kept 4 weeks: a second layer under the dumps.
gcloud compute resource-policies create snapshot-schedule agri-weekly --region=REGION \
  --weekly-schedule=sunday --start-time=20:00 --max-retention-days=28
gcloud compute disks add-resource-policies agri-pgdata --zone=ZONE --resource-policies=agri-weekly
```

The address the Cloudflare records point at (step 9): `gcloud compute addresses describe agri-ip --region=REGION --format='value(address)'`.

## 5. Prepare the VM

```bash
gcloud compute ssh agri-vm --zone=ZONE --tunnel-through-iap
```

On the VM:

```bash
# Docker Engine + compose plugin from Docker's repository.
sudo apt-get update && sudo apt-get install -y ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" \
  | sudo tee /etc/apt/sources.list.d/docker.list
sudo apt-get update && sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Format (first time only!) and mount the data disk.
sudo mkfs.ext4 -m 0 /dev/disk/by-id/google-pgdata
sudo mkdir -p /mnt/disks/pgdata
echo "/dev/disk/by-id/google-pgdata /mnt/disks/pgdata ext4 discard,defaults,nofail 0 2" | sudo tee -a /etc/fstab
sudo mount -a && sudo mkdir -p /mnt/disks/pgdata/postgres
```

## 6. Secrets

Values never pass through GitHub; the VM reads them at deploy time.

```bash
openssl rand -hex 32 | gcloud secrets create agri-jwt-secret --data-file=-
openssl rand -hex 24 | gcloud secrets create agri-postgres-password --data-file=-
openssl rand -hex 24 | gcloud secrets create agri-owner-db-password --data-file=-
openssl rand -hex 24 | gcloud secrets create agri-app-db-password --data-file=-
# The first-admin form (/setup) asks for this; the server refuses to start in
# production while there is no admin and no token. Print it once, keep it.
openssl rand -hex 16 | tee /dev/stderr | gcloud secrets create agri-setup-token --data-file=-
# Required in production: without SMTP_HOST the server refuses to start
# (password-reset links would otherwise be written to the log).
printf 'smtp.example.vn' | gcloud secrets create agri-smtp-host --data-file=-
printf '587'             | gcloud secrets create agri-smtp-port --data-file=-
printf 'user'            | gcloud secrets create agri-smtp-username --data-file=-
printf 'pass'            | gcloud secrets create agri-smtp-password --data-file=-
printf 'no-reply@DOMAIN' | gcloud secrets create agri-from-email --data-file=-
# The Cloudflare Origin Certificate Caddy serves (dashboard: SSL/TLS → Origin
# Server → Create certificate, for DOMAIN and *.DOMAIN, PEM). One the domain
# already has can be reused. Deploys refuse to run without both.
gcloud secrets create agri-origin-cert --data-file=origin.pem
gcloud secrets create agri-origin-key --data-file=origin.key && rm origin.key
```

The two database passwords take effect only when the data directory is first created (`db-init/01-app-role.sh`). To rotate one later, change it in Postgres (`ALTER ROLE ... PASSWORD`) and then add a new secret version.

## 7. Buckets: uploads and backups

Product images live in their own private bucket. Deploys refuse to run without it: the container has no persistent disk, so local uploads would vanish on the next deploy.

```bash
gcloud storage buckets create gs://UPLOAD_BUCKET --location=REGION --uniform-bucket-level-access \
  --public-access-prevention
# Deleted or replaced images stay recoverable for 30 days, so a database
# restored from an older dump still finds the images it points at.
gcloud storage buckets update gs://UPLOAD_BUCKET --versioning
echo '{"rule":[{"action":{"type":"Delete"},"condition":{"isLive":false,"daysSinceNoncurrentTime":30}}]}' > /tmp/lc-uploads.json
gcloud storage buckets update gs://UPLOAD_BUCKET --lifecycle-file=/tmp/lc-uploads.json
gcloud storage buckets add-iam-policy-binding gs://UPLOAD_BUCKET \
  --member="serviceAccount:$VM_SA" --role=roles/storage.objectAdmin
```

Images are served through the app (`/files/{id}`), never from the bucket, so it stays private.

Backups:

```bash
gcloud storage buckets create gs://BACKUP_BUCKET --location=REGION --uniform-bucket-level-access
gcloud storage buckets update gs://BACKUP_BUCKET --versioning
echo '{"rule":[{"action":{"type":"Delete"},"condition":{"age":30}}]}' > /tmp/lc.json
gcloud storage buckets update gs://BACKUP_BUCKET --lifecycle-file=/tmp/lc.json
gcloud storage buckets add-iam-policy-binding gs://BACKUP_BUCKET \
  --member="serviceAccount:$VM_SA" --role=roles/storage.objectAdmin
```

On the VM, create the file the backup timer reads. Until it exists, every deploy warns that backups are off:

```bash
sudo install -d -m 700 /etc/agri-connect
echo 'BACKUP_BUCKET=BACKUP_BUCKET' | sudo tee /etc/agri-connect/backup.env
sudo chmod 600 /etc/agri-connect/backup.env
```

## 8. GitHub configuration

Under *Settings → Environments → production*:
- add yourself as a required reviewer, so every deploy waits for a click;
- add these **variables** (none of them are secrets):

| Variable | Value |
|---|---|
| `GCP_PROJECT_ID` | `PROJECT_ID` |
| `GCP_REGION` | `REGION` |
| `ARTIFACT_REPO` | `agri-connect` |
| `GCP_WIF_PROVIDER` | output of `gcloud iam workload-identity-pools providers describe github --location=global --workload-identity-pool=github --format='value(name)'` |
| `GCP_DEPLOY_SERVICE_ACCOUNT` | `agri-deploy@PROJECT_ID.iam.gserviceaccount.com` |
| `GCE_VM` | `agri-vm` |
| `GCE_ZONE` | `ZONE` |
| `DOMAIN` | `DOMAIN` |
| `UPLOAD_BUCKET` | `UPLOAD_BUCKET` (step 7, without `gs://`) |

The first deploy: *Actions → Deploy → Run workflow*.

## 9. Cloudflare

- SSL/TLS mode **Full (strict)**: Cloudflare checks the Origin Certificate.
- DNS: the `@` and `www` A records at the static IP (step 4), **Proxied**; `www` redirects to `DOMAIN`. Mail records (MX and the like) stay DNS only.
- No cache rule for HTML: pages carry the session. After moving the domain from another site, *Caching → Configuration → Purge Everything*.
- Renewing the Origin Certificate (before its expiry) is a new version of `agri-origin-cert` and `agri-origin-key`, then a deploy.

Behind the proxy every connection comes from a Cloudflare edge. Caddy trusts `CF-Connecting-IP` only from Cloudflare's ranges (`Caddyfile`) and hands the server that one address (`TRUSTED_PROXY_HOPS=1`), so rate limits and sign-in lockouts are per visitor. A range missing there fails silently: its visitors share one bucket.

## Day-2 operations

| Task | Command (on the VM, from `/opt/agri-connect`) |
|---|---|
| Status | `sudo docker compose --env-file /etc/agri-connect/compose.env ps` |
| Logs (JSON, one `request_id` per request) | `sudo docker compose --env-file /etc/agri-connect/compose.env logs -f web` |
| Run a backup now | `sudo systemctl start agri-backup.service && journalctl -u agri-backup -n 20` |
| Roll back | re-run *Deploy* for an older commit from the Actions tab |

## Slow queries

Postgres starts with `pg_stat_statements` loaded and with memory settings for the e2-small VM (`compose.yaml`; on a larger VM set `PG_SHARED_BUFFERS`, `PG_EFFECTIVE_CACHE_SIZE`, `PG_WORK_MEM`, `PG_MAINTENANCE_WORK_MEM` in `/etc/agri-connect/compose.env`: about a quarter of the memory for the first, half for the second). A database created before this needs the extension once, as the superuser (new ones get it from `db-init`):

```bash
C="sudo docker compose --env-file /etc/agri-connect/compose.env exec -T db"
$C psql -U postgres -d agri_connect -c "CREATE EXTENSION IF NOT EXISTS pg_stat_statements"
```

Decide an index or a cache from what it measures, never from a guess: an index is paid on every write of its table.

```bash
# What the database spends its time on.
$C psql -U postgres -d agri_connect -c "SELECT round(total_exec_time) AS ms, calls, round(mean_exec_time::numeric, 1) AS mean_ms, left(query, 100) AS query FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 15"
# Indexes nothing has read since the statistics were last reset: candidates to drop.
$C psql -U postgres -d agri_connect -c "SELECT relname, indexrelname, pg_size_pretty(pg_relation_size(indexrelid)) AS size FROM pg_stat_user_indexes WHERE idx_scan = 0 ORDER BY pg_relation_size(indexrelid) DESC"
```

The app cancels a statement after `DB_STATEMENT_TIMEOUT_SECS` (20) and fails a request that waits more than 5 s for a connection (`DB_MAX_CONNECTIONS`, 20): both show in the `web` log as a database error with the request's id.

## Restore

Practise this once before launch; a backup nobody has restored is a guess. The
steps below were rehearsed against a local PostgreSQL + PostGIS with the
earlier single app role: the restored database had the same roles, grants,
rows and indexes, and the server started on it without re-seeding anything.
The two-role version below (agri_owner, agri_app) has not been rehearsed
yet; do it once before launch.

```bash
# 1. Pick a dump.
gcloud storage ls gs://BACKUP_BUCKET/postgres/
# 2. Stop the app so nothing writes during the restore.
sudo docker compose --env-file /etc/agri-connect/compose.env stop web
# 3. Recreate the database exactly as db-init does: extensions belong to the
#    postgres superuser, the schema to agri_owner, rows to agri_app through
#    the owner's default privileges. The roles already exist (they belong to
#    the server, not the database), so only the database-level lines run.
C="sudo docker compose --env-file /etc/agri-connect/compose.env exec -T db"
$C psql -U postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE agri_connect WITH (FORCE)" \
  -c "CREATE DATABASE agri_connect OWNER agri_owner"
$C psql -U postgres -d agri_connect -v ON_ERROR_STOP=1 \
  -c "ALTER SCHEMA public OWNER TO agri_owner" \
  -c "REVOKE CREATE ON SCHEMA public FROM PUBLIC" \
  -c "REVOKE ALL ON DATABASE agri_connect FROM PUBLIC" \
  -c "GRANT CONNECT, TEMPORARY ON DATABASE agri_connect TO agri_app" \
  -c "GRANT USAGE ON SCHEMA public TO agri_app" \
  -c "ALTER DEFAULT PRIVILEGES FOR ROLE agri_owner IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO agri_app" \
  -c "ALTER DEFAULT PRIVILEGES FOR ROLE agri_owner IN SCHEMA public GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO agri_app" \
  -c "CREATE EXTENSION postgis" -c "CREATE EXTENSION unaccent" -c "CREATE EXTENSION pg_trgm" \
  -c "CREATE EXTENSION pg_stat_statements"
# 4. Restore the app's objects as agri_owner, so agri_app gets its rights on
#    them. The dump's CREATE EXTENSION IF NOT EXISTS lines are no-ops now;
#    --no-comments skips COMMENT ON EXTENSION, which only the extension
#    owner may run.
gcloud storage cat gs://BACKUP_BUCKET/postgres/agri_connect-YYYYMMDDTHHMMSSZ.dump \
  | $C pg_restore -U agri_owner -d agri_connect --no-owner --no-comments --exit-on-error
# 5. Start the app; startup migrations and catalogue seeding are idempotent.
sudo docker compose --env-file /etc/agri-connect/compose.env start web
```

Images are not in the dump. A restored database points at images that existed when the dump was taken; any deleted since are noncurrent versions in the uploads bucket (kept 30 days) and can be restored with `gcloud storage cp` from their generation.

If the disk itself is lost, restore the latest snapshot of `agri-pgdata` to a new disk, attach it as `pgdata`, and mount it as in step 5 (skip `mkfs`).
