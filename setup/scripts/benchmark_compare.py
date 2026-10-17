#!/usr/bin/env python3
"""Compare two criterion result homes and emit a markdown report.

Criterion writes one ``<bench id>/new/estimates.json`` per benchmark under
``CRITERION_HOME``. This script pairs the two homes by bench id, compares the
median point estimates, and writes:

* a markdown table for humans (PR comment, step summary), and
* a JSON status object the benchmark workflow gates on.

A benchmark counts as a regression when the head median is more than
``--threshold-pct`` percent slower than the base median. Benchmarks present
on only one side are reported as new or removed and never gate.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def load_medians(home: Path) -> dict[str, float]:
	"""Map each benchmark id to its median point estimate, in nanoseconds."""
	medians = {}

	for estimates in sorted(home.glob("**/new/estimates.json")):
		data = json.loads(estimates.read_text())
		point_estimate = data.get("median", {}).get("point_estimate")

		if point_estimate is not None:
			bench_id = str(estimates.relative_to(home).parent.parent)
			medians[bench_id] = float(point_estimate)

	return medians


def format_duration(nanoseconds: float) -> str:
	"""Render a criterion estimate without false precision."""
	if nanoseconds >= 1_000_000:
		return f"{nanoseconds / 1_000_000:.2f} ms"

	if nanoseconds >= 1_000:
		return f"{nanoseconds / 1_000:.2f} µs"

	return f"{nanoseconds:.0f} ns"


def main() -> int:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--base", type=Path, required=True, help="criterion home of the base ref")
	parser.add_argument("--head", type=Path, required=True, help="criterion home of the head ref")
	parser.add_argument(
		"--threshold-pct",
		type=float,
		default=10.0,
		help="median slowdown (percent) beyond which a benchmark gates",
	)
	parser.add_argument("--markdown", type=Path, required=True, help="where to write the report")
	parser.add_argument("--status", type=Path, required=True, help="where to write the status JSON")
	args = parser.parse_args()

	base = load_medians(args.base)
	head = load_medians(args.head)

	rows = []
	regressions = 0
	improvements = 0

	for bench_id in sorted(set(base) | set(head)):
		if bench_id in base and bench_id not in head:
			rows.append((bench_id, format_duration(base[bench_id]), "—", "—", "removed"))

			continue

		if bench_id not in base:
			rows.append((bench_id, "—", format_duration(head[bench_id]), "—", "new"))

			continue

		base_median = base[bench_id]
		head_median = head[bench_id]
		change_pct = (head_median - base_median) / base_median * 100

		if change_pct > args.threshold_pct:
			verdict = "regression"
			regressions += 1
		elif change_pct < -args.threshold_pct:
			verdict = "improved"
			improvements += 1
		else:
			verdict = "unchanged"

		rows.append(
			(
				bench_id,
				format_duration(base_median),
				format_duration(head_median),
				f"{change_pct:+.1f}%",
				verdict,
			)
		)

	lines = [
		"| Benchmark | Base | Head | Change | Verdict |",
		"| --- | --- | --- | --- | --- |",
	]

	for bench_id, base_value, head_value, change, verdict in rows:
		lines.append(f"| `{bench_id}` | {base_value} | {head_value} | {change} | {verdict} |")

	args.markdown.write_text("\n".join(lines) + "\n")

	args.status.write_text(
		json.dumps(
			{
				"regressions": regressions,
				"improvements": improvements,
				"benchmark_count": len(rows),
				"threshold_pct": args.threshold_pct,
			},
			indent="\t",
		)
		+ "\n"
	)

	print("\n".join(lines))

	return 0


if __name__ == "__main__":
	raise SystemExit(main())
