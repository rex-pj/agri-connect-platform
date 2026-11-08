-- Builds admin_units.tsv (gzipped to admin_units.tsv.gz) from the
-- vietnamese-provinces-database tables loaded in a scratch database: the
-- core tables (provinces, wards) and the GIS add-on (gis_provinces,
-- gis_wards). See README.md for the commands.
--
-- Wards are one coverage: ST_CoverageClean first removes the source's
-- overlaps (neighbours drawn slightly differently), then ST_CoverageSimplify
-- drops vertices while every shared edge stays shared, so simplifying opens
-- no gap and makes no overlap. ST_ReducePrecision snaps to 1e-5 degrees
-- (about 1 m), the TWKB precision written, and always returns valid shapes.
--
-- Needs PostGIS 3.6 (GEOS 3.14) for ST_CoverageClean. Only this build does;
-- the server reads the file with ST_GeomFromTWKB, in every PostGIS since 2.2.

\set ON_ERROR_STOP on
\set tolerance 0.0002

DROP TABLE IF EXISTS build_wards;
CREATE TABLE build_wards AS
WITH cleaned AS (
    SELECT ward_code, ST_CoverageClean(geom, 0, 0.000002) OVER () AS g FROM gis_wards
), simplified AS (
    SELECT ward_code, ST_CoverageSimplify(g, :tolerance) OVER () AS g FROM cleaned
)
SELECT ward_code,
       ST_Multi(ST_CollectionExtract(ST_ReducePrecision(g, 0.00001), 3)) AS g
FROM simplified;

DO $$
DECLARE bad int;
BEGIN
    SELECT count(*) INTO bad FROM build_wards
    WHERE g IS NULL OR ST_IsEmpty(g) OR NOT ST_IsValid(g);
    IF bad > 0 THEN RAISE EXCEPTION '% wards are empty or invalid', bad; END IF;
    SELECT count(*) INTO bad FROM wards w
    WHERE NOT EXISTS (SELECT 1 FROM build_wards b WHERE b.ward_code = w.code);
    IF bad > 0 THEN RAISE EXCEPTION '% wards have no boundary', bad; END IF;
END $$;

-- The file: a version line, then one line per province and per ward.
--   V  <version>
--   P  code  name  full_name  unit_type  lat  lng
--   W  code  province_code  name  full_name  unit_type  lat  lng  twkb_hex
-- Unit types: province, city (provinces); ward, commune, special_zone.
-- The point is ST_PointOnSurface: always inside the unit.
-- \copy does not expand psql variables: the version goes through a table.
DROP TABLE IF EXISTS build_version;
CREATE TABLE build_version AS
SELECT dataset_version || '+t' || :'tolerance' AS v FROM vn_provinces_metadata;
\copy (SELECT 'V', v FROM build_version) TO 'admin_units_version.tsv' WITH (FORMAT text)
\copy (SELECT 'P', p.code, p.name, p.full_name, CASE p.administrative_unit_id WHEN 1 THEN 'city' ELSE 'province' END, round(ST_Y(g.c)::numeric, 5)::text, round(ST_X(g.c)::numeric, 5)::text FROM provinces p JOIN (SELECT province_code, ST_PointOnSurface(geom) c FROM gis_provinces) g ON g.province_code = p.code ORDER BY p.code) TO 'admin_units_provinces.tsv' WITH (FORMAT text)
\copy (SELECT 'W', w.code, w.province_code, w.name, w.full_name, CASE w.administrative_unit_id WHEN 3 THEN 'ward' WHEN 4 THEN 'commune' ELSE 'special_zone' END, round(ST_Y(ST_PointOnSurface(b.g))::numeric, 5)::text, round(ST_X(ST_PointOnSurface(b.g))::numeric, 5)::text, encode(ST_AsTWKB(b.g, 5), 'hex') FROM wards w JOIN build_wards b ON b.ward_code = w.code ORDER BY w.code) TO 'admin_units_wards.tsv' WITH (FORMAT text)
