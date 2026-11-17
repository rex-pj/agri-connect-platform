#!/bin/sh
# Runs once, when the data directory is empty (postgres image convention).
# Creates the two roles and the extensions (roles.psql): agri_owner owns the
# schema and runs the migrations, agri_app reads and writes rows and nothing
# more. The .psql file is not a .sql one, so the image does not run it again
# on its own.
set -eu

psql -v ON_ERROR_STOP=1 \
    -v owner_role=agri_owner -v owner_pw="$OWNER_DB_PASSWORD" \
    -v app_role=agri_app -v app_pw="$APP_DB_PASSWORD" \
    -v db_name="$POSTGRES_DB" \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    -f /docker-entrypoint-initdb.d/roles.psql
