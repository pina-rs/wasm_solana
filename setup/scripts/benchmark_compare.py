#!/usr/bin/env python3
"""Compare order-balanced criterion runs and emit a markdown report.

Criterion writes one ``<bench id>/new/estimates.json`` per benchmark under
``CRITERION_HOME``. The benchmark workflow runs each binary twice, swapping
which revision goes first:

* base, then head (``--base-first`` / ``--head-second``)
* head, then base (``--head-first`` / ``--base-second``)

A later process on a shared runner is often slower or faster than an earlier
one, by more than the gate's threshold, even when the binaries are identical.
That bias flips sign when the order flips. A benchmark is a regression only
when both orders are slower than ``--threshold-pct`` and the smaller slowdown
is larger than the gap between the two runs of the same binary. The mirror
image is an improvement. Anything else is inconclusive and does not gate.
Benchmarks present on only one revision are new or removed and do not gate.
A run directory with no estimates fails the script.
"""

from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass
from json import JSONDecodeError
from json import dumps
from json import loads
from pathlib import Path
from shutil import copy2


def load_medians(home: Path) -> dict[str, float]:
	"""Map each benchmark id to its median point estimate, in nanoseconds."""
	medians: dict[str, float] = {}

	for estimates in sorted(home.glob("**/new/estimates.json")):
		data = loads(estimates.read_text())
		point_estimate = data.get("median", {}).get("point_estimate")

		if point_estimate is not None:
			bench_id = str(estimates.relative_to(home).parent.parent)
			medians[bench_id] = float(point_estimate)

	return medians


def copy_bench_executables(log: Path, dest: Path) -> list[Path]:
	"""Copy bench binaries named in a ``cargo --message-format=json`` log."""
	dest.mkdir(parents=True, exist_ok=True)
	copied: list[Path] = []
	seen: set[Path] = set()

	for line in log.read_text().splitlines():
		stripped = line.strip()

		if not stripped.startswith("{"):
			continue

		try:
			message = loads(stripped)
		except JSONDecodeError:
			continue

		if message.get("reason") != "compiler-artifact":
			continue

		kinds = message.get("target", {}).get("kind") or []

		if "bench" not in kinds:
			continue

		executable = message.get("executable")

		if not executable:
			continue

		source = Path(executable)

		if source in seen or not source.is_file():
			continue

		seen.add(source)
		target = dest / source.name
		copy2(source, target)
		target.chmod(0o755)
		copied.append(target)

	return copied


def format_duration(nanoseconds: float) -> str:
	"""Render a criterion estimate without false precision."""
	if nanoseconds >= 1_000_000:
		return f"{nanoseconds / 1_000_000:.2f} ms"

	if nanoseconds >= 1_000:
		return f"{nanoseconds / 1_000:.2f} µs"

	return f"{nanoseconds:.0f} ns"


def percent_change(new: float, old: float) -> float:
	"""Percent change of ``new`` relative to ``old``."""
	if old == 0:
		if new == 0:
			return 0.0

		return math.inf if new > 0 else -math.inf

	return (new - old) / old * 100.0


def repeat_noise_pct(first: float, second: float) -> float:
	"""Percent gap between two repeats of one binary, relative to the smaller."""
	low = min(first, second)
	high = max(first, second)

	if low == 0:
		return 0.0 if high == 0 else math.inf

	return (high - low) / low * 100.0


def classify_pair(
	base_first: float,
	head_second: float,
	head_first: float,
	base_second: float,
	threshold_pct: float,
) -> str:
	"""Classify one benchmark from the two run orders and their repeat gap."""
	change_a = percent_change(head_second, base_first)
	change_b = percent_change(head_first, base_second)
	noise = max(
		repeat_noise_pct(base_first, base_second),
		repeat_noise_pct(head_first, head_second),
	)

	if (
		change_a > threshold_pct
		and change_b > threshold_pct
		and min(change_a, change_b) > noise
	):
		return "regression"

	if (
		change_a < -threshold_pct
		and change_b < -threshold_pct
		and min(-change_a, -change_b) > noise
	):
		return "improved"

	if abs(change_a) <= threshold_pct and abs(change_b) <= threshold_pct:
		return "unchanged"

	return "inconclusive"


def format_percent(value: float) -> str:
	"""Render a signed percent, including a non-finite change."""
	if not math.isfinite(value):
		return "+inf%" if value > 0 else "-inf%"

	return f"{value:+.1f}%"


@dataclass(frozen=True)
class Row:
	bench_id: str
	base: str
	head: str
	base_then_head: str
	head_then_base: str
	noise: str
	verdict: str


@dataclass(frozen=True)
class Report:
	rows: list[Row]
	regressions: int
	improvements: int
	inconclusive: int


def _mean(values: list[float]) -> float:
	return sum(values) / len(values)


def compare_medians(
	base_first: dict[str, float],
	head_second: dict[str, float],
	head_first: dict[str, float],
	base_second: dict[str, float],
	threshold_pct: float,
) -> Report:
	"""Pair four median maps into a report. Incomplete benches do not gate."""
	runs = (base_first, head_second, head_first, base_second)
	rows: list[Row] = []
	regressions = 0
	improvements = 0
	inconclusive = 0

	for bench_id in sorted(set().union(*runs)):
		in_base_first = bench_id in base_first
		in_head_second = bench_id in head_second
		in_head_first = bench_id in head_first
		in_base_second = bench_id in base_second
		base_points = [
			points[bench_id]
			for points, is_present in (
				(base_first, in_base_first),
				(base_second, in_base_second),
			)
			if is_present
		]
		head_points = [
			points[bench_id]
			for points, is_present in (
				(head_second, in_head_second),
				(head_first, in_head_first),
			)
			if is_present
		]
		base_cell = format_duration(_mean(base_points)) if base_points else "—"
		head_cell = format_duration(_mean(head_points)) if head_points else "—"

		if in_base_first and in_head_second and in_head_first and in_base_second:
			verdict = classify_pair(
				base_first[bench_id],
				head_second[bench_id],
				head_first[bench_id],
				base_second[bench_id],
				threshold_pct,
			)
			noise = max(
				repeat_noise_pct(base_first[bench_id], base_second[bench_id]),
				repeat_noise_pct(head_first[bench_id], head_second[bench_id]),
			)
			row = Row(
				bench_id,
				base_cell,
				head_cell,
				format_percent(percent_change(head_second[bench_id], base_first[bench_id])),
				format_percent(percent_change(head_first[bench_id], base_second[bench_id])),
				format_percent(noise).removeprefix("+"),
				verdict,
			)
		elif in_head_second and in_head_first and not in_base_first and not in_base_second:
			verdict = "new"
			row = Row(bench_id, "—", head_cell, "—", "—", "—", verdict)
		elif in_base_first and in_base_second and not in_head_second and not in_head_first:
			verdict = "removed"
			row = Row(bench_id, base_cell, "—", "—", "—", "—", verdict)
		else:
			verdict = "inconclusive"
			row = Row(bench_id, base_cell, head_cell, "—", "—", "—", verdict)

		if verdict == "regression":
			regressions += 1
		elif verdict == "improved":
			improvements += 1
		elif verdict == "inconclusive":
			inconclusive += 1

		rows.append(row)

	return Report(rows, regressions, improvements, inconclusive)


def render_markdown(report: Report) -> str:
	"""Render the report as a markdown table."""
	lines = [
		"| Benchmark | Base | Head | Base then head | Head then base | Repeat noise | Verdict |",
		"| --- | --- | --- | --- | --- | --- | --- |",
	]

	for row in report.rows:
		lines.append(
			f"| `{row.bench_id}` | {row.base} | {row.head} | {row.base_then_head} | {row.head_then_base} | {row.noise} | {row.verdict} |"
		)

	return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--base-first", type=Path, required=True, help="criterion home of the base run that went first")
	parser.add_argument("--head-second", type=Path, required=True, help="criterion home of the head run that went second")
	parser.add_argument("--head-first", type=Path, required=True, help="criterion home of the head run that went first")
	parser.add_argument("--base-second", type=Path, required=True, help="criterion home of the base run that went second")
	parser.add_argument(
		"--threshold-pct",
		type=float,
		default=10.0,
		help="slowdown (percent) beyond which a benchmark can gate",
	)
	parser.add_argument("--markdown", type=Path, required=True, help="where to write the report")
	parser.add_argument("--status", type=Path, required=True, help="where to write the status JSON")
	args = parser.parse_args(argv)

	homes = {
		"--base-first": args.base_first,
		"--head-second": args.head_second,
		"--head-first": args.head_first,
		"--base-second": args.base_second,
	}
	medians = {flag: load_medians(home) for flag, home in homes.items()}
	empty = [flag for flag, points in medians.items() if not points]

	if empty:
		print(
			f"criterion run produced no estimates: {', '.join(empty)}",
			file=sys.stderr,
		)

		return 1

	report = compare_medians(
		medians["--base-first"],
		medians["--head-second"],
		medians["--head-first"],
		medians["--base-second"],
		args.threshold_pct,
	)
	markdown = render_markdown(report)
	args.markdown.write_text(markdown)
	args.status.write_text(
		dumps(
			{
				"regressions": report.regressions,
				"improvements": report.improvements,
				"inconclusive": report.inconclusive,
				"benchmark_count": len(report.rows),
				"threshold_pct": args.threshold_pct,
			},
			indent="\t",
		)
		+ "\n"
	)
	print(markdown, end="")

	return 0


if __name__ == "__main__":
	raise SystemExit(main())
