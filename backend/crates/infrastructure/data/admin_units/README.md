# Provinces and wards

`admin_units.tsv.gz` holds Vietnam's administrative units since 1 July 2025:
34 provinces and 3 321 wards, communes and special zones, with their
official codes, a point inside each, and each ward's boundary. The server
embeds the file and loads it on startup when its version is not already in
the database (`AdminUnitRepository::sync`, recorded in `catalogue_state`).
There are no hamlets (thôn, ấp): no open national list exists, and the
provinces are still merging them.

## Source

[thanglequoc/vietnamese-provinces-database](https://github.com/thanglequoc/vietnamese-provinces-database)
(MIT), release **v5.2.0**, commit `62b1abdfe39625289c37a58ef6678e9db3d37f84`:
codes and names from the General Statistics Office, up to resolution
388/NQ-UBTVQH16 and 39/2026/QH16 (Bắc Ninh a city, 20 September 2026).
Boundaries are its GIS add-on (from sapnhap.bando.com.vn).

## What the build changes

The source boundaries take 116 MB (7.1 million vertices). The build keeps
4.4 MB:

1. `ST_CoverageClean` removes the source's overlaps between neighbours.
2. `ST_CoverageSimplify` at 0.0002° drops vertices while every shared edge
   stays shared, so no gap or overlap appears between two wards.
3. `ST_ReducePrecision` to 1e-5° (about 1 m), written as TWKB.

Checked against the source on 200 000 random points: 0.04 % land in another
ward, all of them within 7 m of the true boundary (less than a phone's GPS
error). A point just outside every ward counts under the nearest one within
`ADMIN_UNIT_SNAP_M`.

| Tolerance | Vertices | TWKB | Points in another ward | Furthest from the boundary |
| --------- | -------- | ---- | ---------------------- | -------------------------- |
| 0.0001°   | 2.4 M    | 5.6 MB | 0.018 % | 3 m  |
| **0.0002°** | **1.5 M** | **4.2 MB** | **0.040 %** | **7 m** |
| 0.0005°   | 0.8 M    | 2.6 MB | 0.129 % | 26 m |

## Rebuilding (a new release)

Needs PostgreSQL with PostGIS 3.6+ (GEOS 3.14, for `ST_CoverageClean`); the
server itself only needs `ST_GeomFromTWKB`.

```sh
# A scratch database, never the app's.
createdb vn_admin_src
export PGCLIENTENCODING=UTF8   # the source SQL is UTF-8
BASE=https://raw.githubusercontent.com/thanglequoc/vietnamese-provinces-database/<commit>/postgresql
for f in postgres_CreateTables_vn_units.sql postgres_ImportData_vn_units.sql \
         gis/postgresql_CreateGISTables.sql gis/postgresql_ImportData_gis-part-0{1,2,3,4}.sql; do
  curl -sfL "$BASE/$f" | psql -d vn_admin_src -v ON_ERROR_STOP=1 -q
done

# Writes admin_units_{version,provinces,wards}.tsv in the current directory.
psql -d vn_admin_src -f build.sql
cat admin_units_version.tsv admin_units_provinces.tsv admin_units_wards.tsv \
  | gzip -9 -n > admin_units.tsv.gz
```

The version line (`v5.2.0+t0.0002`) changes with the release or the
tolerance; the next startup reloads names, points and boundaries, turns off
units no longer listed (rows that refer to them keep working) and never
deletes one. Then run `cargo test -p infrastructure` (the file's own test
counts the units).
