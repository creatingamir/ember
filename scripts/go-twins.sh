#!/usr/bin/env bash
# Runs every Go twin in tests/golden and writes its stdout to the matching .out file.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
suite_dir="$repo_root/tests/golden"

# Without nullglob, an empty suite would leave the literal pattern in the loop.
shopt -s nullglob
for go_file in "$suite_dir"/*.go; do
    out_file="${go_file%.go}.out"
    echo "go run ${go_file#"$repo_root"/} > ${out_file#"$repo_root"/}"
    go run "$go_file" > "$out_file"
done
