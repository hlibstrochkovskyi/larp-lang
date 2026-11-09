#!/usr/bin/env python3
"""Require both intended preview hosts to have executed the identical candidate."""

import argparse
import hashlib
import json
from pathlib import Path
import re

from version_contract import validate_identity, validate_preview_evidence
from check_project_workflow import STAGES, RUN_STAGES, expected_commands

ROOT = Path(__file__).resolve().parents[1]
HOSTS = {"ubuntu-24.04": "24.04", "ubuntu-26.04": "26.04"}
MODES = [[], ["--gc-stress"], ["--artifacts", "--gc-stress"],
         ["--formatted", "--artifacts", "--gc-stress"]]
PROJECT_MODES = ["source", "source-gc-stress", "artifact", "artifact-gc-stress",
                 "formatted-artifact-gc-stress"]


def validate_projects(projects: dict, binary_sha256: str) -> None:
    """Require actual project execution, bound to one binary and fixture inventory."""
    if (not isinstance(projects, dict) or projects.get("status") != "passed"
            or projects.get("binary_sha256") != binary_sha256
            or not re.fullmatch(r"[0-9a-f]{64}", projects.get("manifest_sha256", ""))):
        raise ValueError("Missing or inconsistent packaged project evidence")
    cases = projects.get("cases")
    count = projects.get("project_count")
    if (type(count) is not int or count < 1 or not isinstance(cases, list) or len(cases) != count
            or type(projects.get("vm_executions")) is not int or projects["vm_executions"] != count * 5):
        raise ValueError("Incomplete packaged project execution counts")
    names = set()
    for case in cases:
        if (not isinstance(case, dict) or not isinstance(case.get("id"), str)
                or not re.fullmatch(r"[a-z][a-z0-9-]*", case["id"])
                or case["id"] in names or case.get("status") != "passed"
                or case.get("modes") != PROJECT_MODES):
            raise ValueError("Incomplete or repeated packaged project case")
        names.add(case["id"])



def validate_workflow(workflow: dict, binary_sha256: str) -> None:
    """Require generated and sibling projects to execute every declared CLI step."""
    if (not isinstance(workflow, dict) or workflow.get("schema_version") != 1
            or workflow.get("status") != "passed" or workflow.get("binary_sha256") != binary_sha256
            or not re.fullmatch(r"[0-9a-f]{64}", workflow.get("driver_sha256", ""))
            or type(workflow.get("vm_executions")) is not int
            or workflow["vm_executions"] != len(STAGES) * len(RUN_STAGES)):
        raise ValueError("Missing or inconsistent manifest project workflow evidence")
    cases = workflow.get("cases")
    if (not isinstance(cases, list) or len(cases) != len(STAGES)
            or [case.get("id") for case in cases if isinstance(case, dict)] != list(STAGES)):
        raise ValueError("Manifest project workflow cases are incomplete")
    for case in cases:
        identifier = case["id"]
        sources = case.get("sources")
        expected_sources = {"src/main.larp", "src/aura.larp" if identifier == "scaffold" else "lib/aura.larp"}
        if (case.get("status") != "passed" or case.get("sources_unavailable") is not True
                or not re.fullmatch(r"[0-9a-f]{64}", case.get("manifest_sha256", ""))
                or not isinstance(sources, dict) or set(sources) != expected_sources
                or any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value)
                       for value in sources.values())
                or case.get("commands") != expected_commands(identifier)):
            raise ValueError("Manifest project workflow lacks complete successful execution evidence")


def validate(reports: dict, revision: str) -> dict:
    """Validate independently produced compatibility reports, retaining host details."""
    if set(reports) != set(HOSTS):
        raise ValueError("Exactly the Ubuntu 24.04 and 26.04 compatibility reports are required")
    common = None
    hosts = {}
    fields = ("revision", "archive", "archive_sha256", "binary_sha256", "version", "target",
              "rustc", "cargo", "checker_sha256", "checksummed_files", "conformance_cases", "identity", "historical_versions", "preview", "projects", "project_workflow")
    for label, release in HOSTS.items():
        report = reports[label]
        if (report.get("schema_version") != 1 or report.get("status") != "passed"
                or report.get("revision") != revision or report.get("payload_unchanged") is not True
                or report.get("target") != "x86_64-unknown-linux-gnu"):
            raise ValueError(f"Compatibility did not pass for the expected revision/target: {label}")
        for key in ("archive_sha256", "binary_sha256", "checker_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", report.get(key, "")):
                raise ValueError(f"Missing or invalid {key}: {label}")
        identity = report.get("identity")
        if not isinstance(identity, dict) or not isinstance(identity.get("language_spec"), str):
            raise ValueError(f"Compatibility report is missing its version identity: {label}")
        validate_identity(identity, report["version"], identity["language_spec"])
        validate_preview_evidence(identity, report.get("preview"))
        validate_projects(report.get("projects"), report["binary_sha256"])
        validate_workflow(report.get("project_workflow"), report["binary_sha256"])
        cases = report.get("conformance_cases")
        if report.get("historical_versions") != report.get("identity", {}).get("artifact", {}).get("read"):
            raise ValueError(f"Historical artifact execution does not cover declared read versions: {label}")
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
