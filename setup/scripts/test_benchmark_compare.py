#!/usr/bin/env python3
"""The criterion gate must ignore run-order noise and still catch a real slowdown."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import benchmark_compare as compare


def estimates(point: float) -> str:
	return json.dumps({"median": {"point_estimate": point}})


def write_run(home: Path, medians: dict[str, float]) -> None:
	for bench_id, point in medians.items():
		path = home / bench_id / "new" / "estimates.json"
		path.parent.mkdir(parents=True, exist_ok=True)
		path.write_text(estimates(point))


class CopyExecutablesTest(unittest.TestCase):
	def test_copies_bench_artifacts_and_skips_libraries(self) -> None:
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			binary = root / "client-hash"
			binary.write_text("#!/bin/sh\necho ok\n")
			binary.chmod(0o755)
			library = root / "libwasm.rlib"
			library.write_text("lib")
			log = root / "cargo.json"
			log.write_text(
				"\n".join(
					[
						"compiling something",
						json.dumps(
							{
								"reason": "compiler-artifact",
								"target": {"kind": ["lib"]},
								"executable": str(library),
							}
						),
						json.dumps(
							{
								"reason": "compiler-artifact",
								"target": {"kind": ["bench"]},
								"executable": str(binary),
							}
						),
						json.dumps(
							{
								"reason": "compiler-artifact",
								"target": {"kind": ["bench"]},
								"executable": None,
							}
						),
						"not-json",
					]
				)
			)
			dest = root / "bin"

			copied = compare.copy_bench_executables(log, dest)

			self.assertEqual(copied, [dest / "client-hash"])
			self.assertTrue((dest / "client-hash").is_file())
			self.assertEqual((dest / "client-hash").read_text(), binary.read_text())


class ClassifyPairTest(unittest.TestCase):
	def test_identical_medians_are_unchanged(self) -> None:
		verdict = compare.classify_pair(100, 100, 100, 100, 10)

		self.assertEqual(verdict, "unchanged")

	def test_both_orders_slow_past_threshold_and_noise_is_a_regression(self) -> None:
		# +25% and +24.5%, with about 2% between repeats of each binary.
		verdict = compare.classify_pair(100, 125, 127, 102, 10)

		self.assertEqual(verdict, "regression")

	def test_opposite_orders_are_inconclusive(self) -> None:
		# The second process is slower whichever binary it is.
		verdict = compare.classify_pair(100, 140, 80, 100, 10)

		self.assertEqual(verdict, "inconclusive")

	def test_slowdown_inside_the_repeat_gap_is_inconclusive(self) -> None:
		# Both orders are +30%, but the two repeats of each binary differ by 50%.
		verdict = compare.classify_pair(100, 130, 195, 150, 10)

		self.assertEqual(verdict, "inconclusive")

	def test_large_slowdown_clears_moderate_repeat_noise(self) -> None:
		# Both orders are +80% and the repeats differ by 20%.
		verdict = compare.classify_pair(100, 180, 216, 120, 10)

		self.assertEqual(verdict, "regression")

	def test_both_orders_faster_past_threshold_and_noise_is_improved(self) -> None:
		verdict = compare.classify_pair(100, 70, 73.5, 105, 10)

		self.assertEqual(verdict, "improved")

	def test_second_run_bias_on_identical_code_does_not_regress(self) -> None:
		# Every bench on the assert2 pull request looked slower because the
		# head run was the later process. Reversing the order flips the sign.
		verdict = compare.classify_pair(100, 130, 100, 130, 10)

		self.assertEqual(verdict, "inconclusive")

	def test_real_doubling_survives_a_later_process_slowdown(self) -> None:
		# Head is twice as slow. The later process in each pair is 30% slower.
		verdict = compare.classify_pair(100, 260, 200, 130, 10)

		self.assertEqual(verdict, "regression")

	def test_small_real_change_buried_in_order_bias_does_not_regress(self) -> None:
		# A true +15% with a 30% later-process slowdown disagrees across orders.
		verdict = compare.classify_pair(100, 149.5, 115, 130, 10)

		self.assertEqual(verdict, "inconclusive")

	def test_zero_base_and_positive_head_in_both_orders_is_a_regression(self) -> None:
		verdict = compare.classify_pair(0, 10, 10, 0, 10)

		self.assertEqual(verdict, "regression")


class CompareHomesTest(unittest.TestCase):
	def test_new_and_removed_benches_do_not_gate(self) -> None:
		report = compare.compare_medians(
			{"kept": 100, "gone": 50},
			{"kept": 100, "fresh": 80},
			{"kept": 100, "fresh": 80},
			{"kept": 100, "gone": 50},
			10,
		)

		by_id = {row.bench_id: row.verdict for row in report.rows}

		self.assertEqual(by_id["fresh"], "new")
		self.assertEqual(by_id["gone"], "removed")
		self.assertEqual(by_id["kept"], "unchanged")
		self.assertEqual(report.regressions, 0)

	def test_a_bench_missing_from_one_run_is_inconclusive(self) -> None:
		report = compare.compare_medians(
			{"partial": 100},
			{"partial": 200},
			{"partial": 200},
			{},
			10,
		)

		self.assertEqual(report.rows[0].verdict, "inconclusive")
		self.assertEqual(report.regressions, 0)

	def test_uniform_second_run_slowdown_gates_nothing(self) -> None:
		base = {
			"parse": 1_000.0,
			"wire": 2_000.0,
			"ack": 3_000.0,
		}
		# The later process is 40% slower for every bench, on identical code.
		later = {bench_id: point * 1.4 for bench_id, point in base.items()}
		report = compare.compare_medians(base, later, base, later, 10)

		self.assertEqual(report.regressions, 0)
		self.assertTrue(all(row.verdict == "inconclusive" for row in report.rows))

	def test_report_records_both_orders_and_the_regression_count(self) -> None:
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			homes = {
				"base_first": root / "base-first",
				"head_second": root / "head-second",
				"head_first": root / "head-first",
				"base_second": root / "base-second",
			}
			write_run(homes["base_first"], {"client/dispatch": 100})
			write_run(homes["head_second"], {"client/dispatch": 180})
			write_run(homes["head_first"], {"client/dispatch": 216})
			write_run(homes["base_second"], {"client/dispatch": 120})
			markdown = root / "report.md"
			status = root / "status.json"

			exit_code = compare.main(
				[
					"--base-first",
					str(homes["base_first"]),
					"--head-second",
					str(homes["head_second"]),
					"--head-first",
					str(homes["head_first"]),
					"--base-second",
					str(homes["base_second"]),
					"--threshold-pct",
					"10",
					"--markdown",
					str(markdown),
					"--status",
					str(status),
				]
			)

			self.assertEqual(exit_code, 0)
			payload = json.loads(status.read_text())
			self.assertEqual(payload["regressions"], 1)
			self.assertEqual(payload["threshold_pct"], 10)
			text = markdown.read_text()
			self.assertIn("Base then head", text)
			self.assertIn("Head then base", text)
			self.assertIn("regression", text)

	def test_empty_result_directory_fails(self) -> None:
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			homes = [root / name for name in ("base-first", "head-second", "head-first", "base-second")]

			for home in homes:
				home.mkdir()

			write_run(homes[0], {"client/dispatch": 100})
			write_run(homes[2], {"client/dispatch": 100})
			write_run(homes[3], {"client/dispatch": 100})
			completed = subprocess.run(
				[
					sys.executable,
					str(Path(compare.__file__)),
					"--base-first",
					str(homes[0]),
					"--head-second",
					str(homes[1]),
					"--head-first",
					str(homes[2]),
					"--base-second",
					str(homes[3]),
					"--markdown",
					str(root / "report.md"),
					"--status",
					str(root / "status.json"),
				],
				check=False,
				capture_output=True,
				text=True,
			)

			self.assertNotEqual(completed.returncode, 0)


if __name__ == "__main__":
	unittest.main()
