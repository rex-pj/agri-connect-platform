#!/bin/sh
# Runs once, when the data directory is empty (postgres image convention).
# The app connects as agri_app, which owns the database but is not a
# superuser; extensions are created here because creating them needs one.
set -eu

psql -v ON_ERROR_STOP=1 -v app_pw="$APP_DB_PASSWORD" \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<'SQL'
CREATE ROLE agri_app LOGIN PASSWORD :'app_pw';
ALTER DATABASE agri_connect OWNER TO agri_app;
ALTER SCHEMA public OWNER TO agri_app;
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
SQL
