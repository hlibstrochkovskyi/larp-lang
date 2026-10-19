"""Regression tests for candidate evidence rejection, without executing a LARP CLI."""

import copy
import hashlib
from pathlib import Path
import tempfile
import unittest

from check_candidate import checked_contents
from check_candidate_matrix import MODES, validate

REVISION = "a" * 40


def synthetic_reports() -> dict:
    """Synthetic evidence exercises gate decisions, not actual platform support."""
    reports = {}
    for release in ("24.04", "26.04"):
        reports[f"ubuntu-{release}"] = {
            "schema_version": 1, "status": "passed", "revision": REVISION,
            "archive": "larp-example.tar.gz", "archive_sha256": "b" * 64,
            "binary_sha256": "c" * 64, "checker_sha256": "d" * 64,
            "version": "0.1.0-dev", "target": "x86_64-unknown-linux-gnu",
            "rustc": "synthetic compiler metadata", "cargo": "synthetic Cargo metadata",
            "checksummed_files": 10, "conformance_cases": 2, "payload_unchanged": True,
            "conformance_modes": [{"flags": copy.copy(mode), "passed": 2, "pending": 0} for mode in MODES],
            "environment": {"system": "Linux", "machine": "x86_64", "glibc": "synthetic libc",
                            "os_release": {"ID": "ubuntu", "VERSION_ID": release},
                            "runner_image": {"ImageVersion": "synthetic image"}},
        }
    return reports


class MatrixEvidenceTests(unittest.TestCase):
    def test_matching_candidate_on_two_distinct_hosts(self):
        result = validate(synthetic_reports(), REVISION)
        self.assertEqual(set(result["hosts"]), {"ubuntu-24.04", "ubuntu-26.04"})

    def test_different_archive_or_binary_is_rejected(self):
        for field in ("archive_sha256", "binary_sha256", "checker_sha256"):
            with self.subTest(field=field):
                reports = synthetic_reports()
                reports["ubuntu-26.04"][field] = "e" * 64
                with self.assertRaises(ValueError):
                    validate(reports, REVISION)

    def test_missing_host_is_rejected(self):
        reports = synthetic_reports()
        del reports["ubuntu-26.04"]
        with self.assertRaises(ValueError):
            validate(reports, REVISION)

    def test_two_reports_from_the_same_os_are_rejected(self):
        reports = synthetic_reports()
        reports["ubuntu-26.04"]["environment"]["os_release"]["VERSION_ID"] = "24.04"
        with self.assertRaises(ValueError):
            validate(reports, REVISION)

    def test_old_revision_failed_job_or_missing_mode_is_rejected(self):
        for field, value in (("revision", "f" * 40), ("status", "failed"),
                             ("conformance_modes", []), ("payload_unchanged", False)):
            with self.subTest(field=field):
                reports = synthetic_reports()
                reports["ubuntu-26.04"][field] = value
                with self.assertRaises(ValueError):
                    validate(reports, REVISION)


class PayloadInventoryTests(unittest.TestCase):
    def test_mutation_and_omitted_files_are_rejected(self):
        with tempfile.TemporaryDirectory(prefix="larp-checksum-regression-") as directory:
            root = Path(directory)
            root.joinpath("sample.txt").write_bytes(b"original")
            expected = hashlib.sha256(b"original").hexdigest()
            root.joinpath("SHA256SUMS").write_text(f"{expected}  sample.txt\n", encoding="utf-8")
            self.assertEqual(checked_contents(root), {"sample.txt": expected})
            root.joinpath("omitted.txt").write_bytes(b"not inventoried")
            with self.assertRaises(RuntimeError):
                checked_contents(root)
            root.joinpath("sample.txt").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                checked_contents(root)

    def test_parent_checksum_path_is_rejected_before_reading(self):
        with tempfile.TemporaryDirectory(prefix="larp-checksum-path-") as directory:
            root = Path(directory)
            root.joinpath("SHA256SUMS").write_text(f"{'a' * 64}  ../outside.txt\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "checksum path"):
                checked_contents(root)


if __name__ == "__main__":
    unittest.main()
