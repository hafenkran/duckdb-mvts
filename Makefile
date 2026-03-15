.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=mvts

# Set to 1 to enable Unstable API (binaries will only work on TARGET_DUCKDB_VERSION, forwards compatibility will be broken)
# Note: currently extension-template-rs requires this, as duckdb-rs relies on unstable C API functionality
USE_UNSTABLE_C_API=1

# Target DuckDB version
TARGET_DUCKDB_VERSION=v1.5.0
DUCKDB_CLI_VERSION ?= $(TARGET_DUCKDB_VERSION)

EXTENSION_FILE := $(PROJ_DIR)build/debug/extension/mvts/mvts.duckdb_extension
DUCKDB_BIN ?= $(PROJ_DIR)testenv/duckdb
IMPORT_SQL ?= $(PROJ_DIR)scripts/import_testenv.sql
TEST_DATA_SENTINEL := $(PROJ_DIR)testenv/nyc-dataset/data/nyc_streets.shp
PORT := 8080
LOG_FILE := $(PROJ_DIR)build/debug/mvts.log
MIN_ZOOM ?= 0
MAX_ZOOM ?= 22
CLIP_GEOMETRIES ?= false

all: configure debug

# Include makefiles from DuckDB
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

# Rust unit tests
test_rust_unit:
	cargo test --lib

test_rust_unit_release:
	cargo test --lib --release

test: test_debug
test_debug: test_rust_unit test_extension_debug
test_release: test_rust_unit_release test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean

.PHONY: debug-run
debug-run: debug run

.PHONY: release-run
release-run: release run

.PHONY: prepare-testenv
prepare-testenv:
	@$(PROJ_DIR)scripts/fetch_nyc_example_dataset.sh
	@$(PROJ_DIR)scripts/fetch_latest_duckdb.sh "$(DUCKDB_CLI_VERSION)"

.PHONY: run
run: $(EXTENSION_FILE)
	@echo "Loading extension, importing data, and starting server on port $(PORT)..."
	@echo "Logging to $(LOG_FILE)"
	@echo "Zoom levels: $(MIN_ZOOM) - $(MAX_ZOOM)"
	@TMP=$$(mktemp); \
	echo "LOAD '$(EXTENSION_FILE)';" > $$TMP; \
	if [ -f "$(TEST_DATA_SENTINEL)" ] && [ -f "$(IMPORT_SQL)" ]; then \
		echo ".read '$(IMPORT_SQL)'" >> $$TMP; \
	else \
		echo "SELECT 'NYC test data missing; skipping import.';" >> $$TMP; \
	fi; \
	echo "SELECT mvts_start($(PORT));" >> $$TMP; \
	echo ".mode column" >> $$TMP; \
	echo "SELECT mvts_status();" >> $$TMP; \
	echo "SELECT 'Server started on port $(PORT). DuckDB session will stay open.';" >> $$TMP; \
	echo "SELECT 'Zoom levels: $(MIN_ZOOM) - $(MAX_ZOOM)';" >> $$TMP; \
	echo "SELECT 'Press Ctrl+C to stop the server and exit.';" >> $$TMP; \
	MVTS_CLIP_GEOMETRIES=$(CLIP_GEOMETRIES) \
		MVTS_LOG_FILE=$(LOG_FILE) \
		MVTS_MIN_ZOOM=$(MIN_ZOOM) \
		MVTS_MAX_ZOOM=$(MAX_ZOOM) \
		$(DUCKDB_BIN) -unsigned -init $$TMP; \
	rm $$TMP
