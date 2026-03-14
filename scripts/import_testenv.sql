-- Import statements for all NYC dataset shapefiles
-- No forced geometry transformation in import: CRS handling happens transparently in MVTS.

-- Ensure spatial extension is available for this DuckDB version/platform
INSTALL spatial;
LOAD spatial;
SET geometry_always_xy = true;

-- nyc_streets
CREATE OR REPLACE TABLE nyc_streets AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/nyc_streets.shp');

-- nyc_census_blocks
CREATE OR REPLACE TABLE nyc_census_blocks AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/nyc_census_blocks.shp');

-- nyc_homicides
CREATE OR REPLACE TABLE nyc_homicides AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/nyc_homicides.shp');

-- nyc_neighborhoods
CREATE OR REPLACE TABLE nyc_neighborhoods AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/nyc_neighborhoods.shp');

-- nyc_subway_stations
CREATE OR REPLACE TABLE nyc_subway_stations AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/nyc_subway_stations.shp');

-- nyc_census_blocks_2000 (in subdirectory)
CREATE OR REPLACE TABLE nyc_census_blocks_2000 AS
        SELECT *
        FROM ST_Read('testenv/nyc-dataset/data/2000/nyc_census_blocks_2000.shp');
