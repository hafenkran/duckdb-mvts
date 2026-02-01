#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
TARGET_BIN="${REPO_DIR}/testenv/duckdb"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required to download DuckDB." >&2
  exit 1
fi

if ! command -v unzip >/dev/null 2>&1; then
  echo "unzip is required to extract DuckDB." >&2
  exit 1
fi

os="$(uname -s)"
arch="$(uname -m)"

asset=""
case "${os}" in
  Linux)
    case "${arch}" in
      x86_64) asset="duckdb_cli-linux-amd64.zip" ;;
      aarch64|arm64) asset="duckdb_cli-linux-aarch64.zip" ;;
    esac
    ;;
  Darwin)
    asset="duckdb_cli-osx-universal.zip"
    ;;
esac

if [[ -z "${asset}" ]]; then
  echo "Unsupported platform: ${os}/${arch}" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

zip_path="${tmp_dir}/${asset}"
extract_dir="${tmp_dir}/extract"

echo "Downloading latest DuckDB CLI (${asset})..."
curl -L -o "${zip_path}" "https://github.com/duckdb/duckdb/releases/latest/download/${asset}"

echo "Extracting DuckDB CLI..."
unzip -q "${zip_path}" -d "${extract_dir}"

duckdb_bin="$(find "${extract_dir}" -type f -name duckdb -print | head -n 1)"
if [[ -z "${duckdb_bin}" ]]; then
  echo "Could not locate DuckDB binary inside the archive." >&2
  exit 1
fi

mkdir -p "$(dirname -- "${TARGET_BIN}")"
cp "${duckdb_bin}" "${TARGET_BIN}"
chmod +x "${TARGET_BIN}"

echo "DuckDB CLI installed at ${TARGET_BIN}"
