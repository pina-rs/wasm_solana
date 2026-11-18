#!/bin/bash

# Build the head and base criterion binaries once, then run them in the order
# base, head, head, base. A later process on a shared runner is often slower
# or faster than an earlier one. Reversing the order makes that bias change
# sign, and the second pair of each binary measures the repeat gap the
# comparison uses as its noise floor.

set -euo pipefail

# Re-exec from outside the worktree. Checking out the base revision replaces
# this script and the comparator, and the second build still has to import it.
if [ -z "${BENCHMARK_RUNTIME:-}" ]; then
	runtime="$(mktemp -d)"
	cp "$0" "$runtime/benchmark_pair.sh"
	cp setup/scripts/benchmark_compare.py "$runtime/benchmark_compare.py"
	export BENCHMARK_RUNTIME="$runtime"
	exec bash "$runtime/benchmark_pair.sh"
fi

if [ -x /usr/bin/python3 ]; then
	python=/usr/bin/python3
else
	python=python3
fi

root="$(pwd)"
package=wasm_client_solana
base_sha="$(cat .benchmark/base-sha)"
head_sha="$(cat .benchmark/head-sha)"

mapfile -t bench_files < <(git ls-tree -r --name-only "$head_sha" -- crates/wasm_client_solana/benches)

if [ "${#bench_files[@]}" -eq 0 ]; then
	echo "no benchmark sources at ${head_sha}" >&2
	exit 1
fi

mkdir -p .benchmark/harness .benchmark/bin

for file in "${bench_files[@]}"; do
	mkdir -p ".benchmark/harness/$(dirname "$file")"
	git show "${head_sha}:${file}" >".benchmark/harness/${file}"
done

copy_executables() {
	local log="$1"
	local dest="$2"

	rm -rf "$dest"
	PYTHONPATH="$BENCHMARK_RUNTIME" "$python" - "$log" "$dest" <<'PY'
import sys
from pathlib import Path

import benchmark_compare

copied = benchmark_compare.copy_bench_executables(Path(sys.argv[1]), Path(sys.argv[2]))

if not copied:
    sys.exit("no benchmark executables in cargo log")

print("\n".join(str(path) for path in copied))
PY
}

build_benches() {
	local dest="$1"
	local log

	log="$(mktemp)"
	cargo bench --locked --package "$package" --features ssr --no-run --message-format=json >"$log"
	copy_executables "$log" "$dest"
	rm -f "$log"
}

run_suite() {
	local bin_dir="$1"
	local home="$2"
	local bin

	export CRITERION_HOME="${root}/${home}"
	rm -rf "$CRITERION_HOME"
	mkdir -p "$CRITERION_HOME"

	for bin in "$bin_dir"/*; do
		[ -f "$bin" ] || continue
		"$bin"
	done
}

# The checked-out tree is the head. Build it before overlaying the harness
# onto the base, and copy the binaries out so the second build can reuse the
# same target directory.
build_benches .benchmark/bin/head

# Historical checkouts must not run devenv's format hooks.
git -c core.hooksPath=/dev/null checkout --force --detach "$base_sha"

for file in "${bench_files[@]}"; do
	mkdir -p "$(dirname "$file")"
	cp ".benchmark/harness/${file}" "$file"
done

build_benches .benchmark/bin/base

git -c core.hooksPath=/dev/null checkout --force --detach "$head_sha"

run_suite .benchmark/bin/base .benchmark/runs/base-first
run_suite .benchmark/bin/head .benchmark/runs/head-second
run_suite .benchmark/bin/head .benchmark/runs/head-first
run_suite .benchmark/bin/base .benchmark/runs/base-second
