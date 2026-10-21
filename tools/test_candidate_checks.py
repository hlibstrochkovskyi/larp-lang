"""Regression tests for candidate evidence rejection, without executing a LARP CLI."""

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from check_candidate import checked_contents
from check_candidate_matrix import MODES, PROJECT_MODES, validate
from version_contract import preview_metadata, validate_identity, version_channel

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
            "identity": {"schema_version": 1, "name": "larp", "version": "0.1.0-dev",
                         "channel": "development", "language_spec": "0.1", "language_status": "experimental",
                         "diagnostics_schema": 1, "artifact": {"write": 2, "read": [1, 2]}},
            "historical_versions": [1, 2],
            "preview": None,
            "projects": {"status": "passed", "binary_sha256": "c" * 64, "manifest_sha256": "e" * 64,
                         "project_count": 1, "vm_executions": 5,
                         "cases": [{"id": "pitch", "status": "passed", "modes": copy.copy(PROJECT_MODES)}]},
            "conformance_modes": [{"flags": copy.copy(mode), "passed": 2, "pending": 0} for mode in MODES],
            "environment": {"system": "Linux", "machine": "x86_64", "glibc": "synthetic libc",
                            "os_release": {"ID": "ubuntu", "VERSION_ID": release},
                            "runner_image": {"ImageVersion": "synthetic image"}},
        }
    return reports


class MatrixEvidenceTests(unittest.TestCase):
    def test_missing_unexecuted_or_wrong_binary_projects_are_rejected(self):
        for change in (None, {"status": "failed"}, {"binary_sha256": "f" * 64},
                       {"vm_executions": 4}, {"project_count": True},
                       {"cases": [{"id": "pitch", "status": "passed", "modes": ["source"]}]}):
            reports = synthetic_reports()
            if change is None:
                reports["ubuntu-26.04"].pop("projects")
            else:
                reports["ubuntu-26.04"]["projects"].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(reports, REVISION)

    def test_different_packaged_project_inventory_is_rejected(self):
        reports = synthetic_reports()
        reports["ubuntu-26.04"]["projects"]["manifest_sha256"] = "f" * 64
        with self.assertRaises(ValueError):
            validate(reports, REVISION)

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

    def test_different_protocol_or_unexecuted_read_version_is_rejected(self):
        for field, value in (("identity", {"artifact": {"read": [1, 2]}, "diagnostics_schema": 2}),
                             ("historical_versions", [2])):
            with self.subTest(field=field):
                reports = synthetic_reports()
                reports["ubuntu-26.04"][field] = value
                with self.assertRaises(ValueError):
                    validate(reports, REVISION)


class VersionIdentityTests(unittest.TestCase):
    def test_semver_prerelease_and_build_metadata_are_distinct(self):
        for version, channel in (("0.1.0-dev", "development"), ("0.1.0-dev.7+local", "development"),
                                 ("0.1.0-preview.1", "preview"), ("0.1.0-rc.1", "prerelease"),
                                 ("0.1.0+preview.1", "release")):
            self.assertEqual(version_channel(version), channel)
        for version in ("01.1.0", "0.1.0-01", "0.1.0-preview..1", "0.1.0-", "0.1.0+", "0.1.0-🦀"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                version_channel(version)

    def test_protocol_types_and_inconsistent_artifact_capabilities_are_rejected(self):
        original = synthetic_reports()["ubuntu-24.04"]["identity"]
        validate_identity(original, "0.1.0-dev", "0.1")
        for field, value in (("schema_version", 2), ("schema_version", True), ("diagnostics_schema", "1"),
                             ("channel", "release"), ("artifact", {"write": 2, "read": [1]}),
                             ("artifact", {"write": True, "read": [1, 2]})):
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                changed = copy.deepcopy(original)
                changed[field] = value
                validate_identity(changed, "0.1.0-dev", "0.1")

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


class PreviewNotesTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[1]

    def identity(self) -> dict:
        identity = copy.deepcopy(synthetic_reports()["ubuntu-24.04"]["identity"])
        identity.update({"version": "0.1.0-preview.1", "channel": "preview"})
        return identity

    def test_prepared_notes_are_bound_to_the_proposed_tag_and_content(self):
        metadata = preview_metadata(self.identity(), lambda name: self.root.joinpath(name).read_bytes())
        self.assertEqual(metadata["proposed_tag"], "v0.1.0-preview.1")
        self.assertGreater(metadata["relative_file_links_checked"], 0)
        notes = self.root.joinpath(metadata["notes"]).read_bytes()
        self.assertEqual(metadata["notes_sha256"], hashlib.sha256(notes).hexdigest())

    def test_wrong_tag_or_boolean_schema_is_rejected(self):
        for field, value in (("proposed_tag", "v0.1.0-preview.2"), ("schema_version", True)):
            with self.subTest(field=field):
                def read_file(name):
                    contents = self.root.joinpath(name).read_bytes()
                    if name.endswith(".json"):
                        descriptor = json.loads(contents)
                        descriptor[field] = value
                        return json.dumps(descriptor).encode()
                    return contents
                with self.assertRaises(ValueError):
                    preview_metadata(self.identity(), read_file)

    def test_zero_preview_number_or_build_metadata_is_not_publication_identity(self):
        for version in ("0.1.0-preview.0", "0.1.0-preview.01", "0.1.0-preview.1+local"):
            identity = self.identity()
            identity["version"] = version
            with self.subTest(version=version), self.assertRaises(ValueError):
                preview_metadata(identity, lambda name: self.fail("Invalid identity must not read files"))

    def test_wrong_note_version_and_parent_link_are_rejected(self):
        for replacement in (b"# LARP 0.1.0-preview.2\n", None):
            def read_file(name):
                contents = self.root.joinpath(name).read_bytes()
                if name.endswith(".md") and name.startswith("docs/releases/"):
                    return replacement if replacement else contents + b"\n[invalid](../../../private.txt)\n"
                return contents
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                preview_metadata(self.identity(), read_file)

    def test_development_channel_does_not_claim_named_preview_checks(self):
        identity = synthetic_reports()["ubuntu-24.04"]["identity"]
        self.assertIsNone(preview_metadata(identity, lambda name: self.fail("Development has no named-preview notes")))

    def test_matrix_requires_preview_document_hashes_even_when_both_reports_agree(self):
        for evidence in (None, {"notes_sha256": "incorrect"}):
            reports = synthetic_reports()
            for report in reports.values():
                report["identity"] = self.identity()
                report["version"] = "0.1.0-preview.1"
                report["preview"] = copy.deepcopy(evidence)
            with self.subTest(evidence=evidence), self.assertRaises(ValueError):
                validate(reports, REVISION)

    def test_matrix_accepts_matching_complete_preview_evidence(self):
        metadata = preview_metadata(self.identity(), lambda name: self.root.joinpath(name).read_bytes())
        reports = synthetic_reports()
        for report in reports.values():
            report["identity"] = self.identity()
            report["version"] = "0.1.0-preview.1"
            report["preview"] = copy.deepcopy(metadata)
        self.assertEqual(validate(reports, REVISION)["preview"], metadata)
        reports["ubuntu-26.04"]["preview"]["notes_sha256"] = "e" * 64
        with self.assertRaises(ValueError):
            validate(reports, REVISION)


if __name__ == "__main__":
    unittest.main()
