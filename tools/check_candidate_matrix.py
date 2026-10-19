#!/usr/bin/env python3
"""Require both intended preview hosts to have executed the identical candidate."""

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
HOSTS = {"ubuntu-24.04": "24.04", "ubuntu-26.04": "26.04"}
MODES = [[], ["--gc-stress"], ["--artifacts", "--gc-stress"],
         ["--formatted", "--artifacts", "--gc-stress"]]


def validate(reports: dict, revision: str) -> dict:
    """Validate independently produced compatibility reports, retaining host details."""
    if set(reports) != set(HOSTS):
        raise ValueError("Exactly the Ubuntu 24.04 and 26.04 compatibility reports are required")
    common = None
    hosts = {}
    fields = ("revision", "archive", "archive_sha256", "binary_sha256", "version", "target",
              "rustc", "cargo", "checker_sha256", "checksummed_files", "conformance_cases")
    for label, release in HOSTS.items():
        report = reports[label]
        if (report.get("schema_version") != 1 or report.get("status") != "passed"
                or report.get("revision") != revision or report.get("payload_unchanged") is not True
                or report.get("target") != "x86_64-unknown-linux-gnu"):
            raise ValueError(f"Compatibility did not pass for the expected revision/target: {label}")
        for key in ("archive_sha256", "binary_sha256", "checker_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", report.get(key, "")):
                raise ValueError(f"Missing or invalid {key}: {label}")
        cases = report.get("conformance_cases")
        if type(cases) is not int or cases < 1 or report.get("conformance_modes") != [
                {"flags": mode, "passed": cases, "pending": 0} for mode in MODES]:
            raise ValueError(f"Incomplete packaged conformance execution: {label}")
        environment = report["environment"]
        if (environment.get("system") != "Linux" or environment.get("machine") != "x86_64"
                or environment["os_release"].get("ID") != "ubuntu"
                or environment["os_release"].get("VERSION_ID") != release
                or not environment.get("glibc") or not environment["runner_image"].get("ImageVersion")):
            raise ValueError(f"Unexpected or unidentified execution host: {label}")
        identity = {key: report[key] for key in fields}
        if common is not None and identity != common:
            raise ValueError("Compatibility jobs did not validate identical candidate/provenance/corpus bytes")
        common = identity
        hosts[label] = environment
    return {**common, "hosts": hosts}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True, help="Downloaded compatibility artifact directories")
    parser.add_argument("--revision", required=True, help="Expected complete source commit")
    parser.add_argument("--output", type=Path, required=True, help="New or empty directory under artifacts/")
    options = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", options.revision):
        parser.error("--revision must be a complete Git commit hash")
    output = options.output.resolve()
    if not output.is_relative_to(ROOT.joinpath("artifacts").resolve()):
        parser.error("Output must stay inside the ignored artifacts/ directory")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output must be new or empty; previous evidence is retained")
    output.mkdir(parents=True, exist_ok=True)
    evidence = {"schema_version": 1, "status": "in_progress", "revision": options.revision,
                "report_hashes": {}}
    try:
        reports = {}
        for label in HOSTS:
            path = options.input / f"candidate-compatibility-{label}" / "compatibility-report.json"
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 8 * 1024 * 1024:
                raise ValueError(f"Missing or oversized compatibility report: {label}")
            data = path.read_bytes()
            evidence["report_hashes"][label] = hashlib.sha256(data).hexdigest()
            reports[label] = json.loads(data)
        evidence.update(validate(reports, options.revision))
        evidence["status"] = "passed"
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        evidence.update({"status": "failed", "error": str(error)})
        raise
    finally:
        output.joinpath("matrix-report.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(f"Both preview hosts executed the same candidate: {evidence['archive_sha256']}")


if __name__ == "__main__":
    main()
