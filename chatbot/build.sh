#!/usr/bin/env bash
# Build the chatbot's sample Parquet dataset with the NATIVE `ocdscardinal export`.
#
# This supersedes build_stopgap.py (the DuckDB-over-flatterer-CSV prototype): the native exporter
# emits the same eight-table schema (analysis/FINDINGS.md Part 5) directly from the OCDS JSONL. See
# chatbot/README.md.
#
# Inputs (already under chatbot/data/_raw/, gitignored):
#   <id>/2026.jsonl        compiled releases (gunzipped)
#   publications.json      the registry index (--registry; whole-dataset metadata)
#   chatbot/meta/<id>.json curated scope prose (--meta)
#   chatbot/settings/<id>.ini  Cardinal settings enabling the indicators (--settings)
#
# Usage: chatbot/build.sh   (run from the repo root)
set -euo pipefail

BIN="target/release/ocdscardinal"
RAW="chatbot/data/_raw"
REGISTRY="$RAW/publications.json"

[ -x "$BIN" ] || { echo "Building release binary..."; cargo build --release; }

# publisher/country are denormalized onto every fact row. Match the registry/curated dataset_meta
# values so the fact-table columns agree with dataset_meta. (dataset_meta itself reads
# publisher/country from the registry + --meta overlay; these flags only fill the fact tables.)
# A `case` is used instead of an associative array for macOS bash 3.2 compatibility.
for id in 145 22; do
  case "$id" in
    145) publisher="Public Procurement Authority (RPPA), Umucyo portal"; country="Rwanda" ;;
    22)  publisher="Direccion General de Contrataciones Publicas (DGCP), Portal transaccional"; country="Dominican Republic" ;;
  esac
  echo "== exporting dataset $id =="
  "$BIN" export "$RAW/$id/2026.jsonl" \
    --output "chatbot/data/$id" \
    --dataset-id "$id" \
    --publisher "$publisher" \
    --country "$country" \
    --year 2026 \
    --registry "$REGISTRY" \
    --meta "chatbot/meta/$id.json" \
    --settings "chatbot/settings/$id.ini"
done

echo "Done. Native Parquet under chatbot/data/<id>/"
