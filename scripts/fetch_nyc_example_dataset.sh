#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
TARGET_DIR="${REPO_DIR}/testenv/nyc-dataset"
DATA_DIR="${TARGET_DIR}/data"
DATA_URL="https://s3.amazonaws.com/s3.cleverelephant.ca/postgis-workshop-2020.zip"

REQUIRED_FILES=(
  "${DATA_DIR}/nyc_streets.shp"
  "${DATA_DIR}/nyc_census_blocks.shp"
  "${DATA_DIR}/nyc_homicides.shp"
  "${DATA_DIR}/nyc_neighborhoods.shp"
  "${DATA_DIR}/nyc_subway_stations.shp"
  "${DATA_DIR}/2000/nyc_census_blocks_2000.shp"
)

missing=false
for path in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "${path}" ]]; then
    missing=true
    break
  fi
done

if [[ "${missing}" == "false" ]]; then
  echo "NYC dataset already present at ${DATA_DIR}"
  exit 0
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required to download the NYC dataset." >&2
  exit 1
fi

if ! command -v unzip >/dev/null 2>&1; then
  echo "unzip is required to extract the NYC dataset." >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

zip_path="${tmp_dir}/postgis-workshop-2020.zip"
extract_dir="${tmp_dir}/extract"

echo "Downloading NYC dataset from ${DATA_URL}..."
curl -L -o "${zip_path}" "${DATA_URL}"

echo "Extracting NYC dataset..."
unzip -q "${zip_path}" -d "${extract_dir}"

source_data_dir="$(find "${extract_dir}" -type d -name data -print | head -n 1)"
if [[ -z "${source_data_dir}" ]]; then
  echo "Could not locate the data directory inside the archive." >&2
  exit 1
fi

mkdir -p "${DATA_DIR}"
cp -R "${source_data_dir}/." "${DATA_DIR}/"

echo "NYC dataset installed into ${DATA_DIR}"
