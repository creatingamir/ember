#!/usr/bin/env bash
# Runs every Go twin under tests/golden, including feature folders, and writes its stdout to the matching .out file.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
suite_dir="$repo_root/tests/golden"

# `find | sort` recurses into feature folders and runs on the bash 3.2 that macOS ships, which has no `globstar`.
# An empty suite gives an empty list, so the loop body never runs.
while IFS= read -r go_file; do
    out_file="${go_file%.go}.out"
    echo "go run ${go_file#"$repo_root"/} > ${out_file#"$repo_root"/}"
    go run "$go_file" > "$out_file"
done < <(find "$suite_dir" -name '*.go' | sort)
