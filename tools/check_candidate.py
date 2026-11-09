#!/usr/bin/env python3
"""Verify and execute one existing LARP candidate without rebuilding its CLI."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import signal
import subprocess
import sys
import tarfile
import tomllib

from version_contract import specification_version, validate_identity
from check_candidate_matrix import validate_projects, validate_workflow
from check_project_workflow import summary as workflow_summary

ROOT = Path(__file__).resolve().parents[1]
MAX_BYTES = 128 * 1024 * 1024
PRIVATE = {"Ideas-not-for push!", ".git", ".git-foundation", ".agents", ".codex", ".aws", "artifacts", "target"}


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def checked_contents(root: Path) -> dict[str, str]:
    """Require the checksum inventory to cover every other regular payload file."""
    inventory = {}
    for line in root.joinpath("SHA256SUMS").read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if match is None:
            raise RuntimeError("Malformed payload checksum entry")
        expected, name = match.groups()
        path = PurePosixPath(name)
        if (path.is_absolute() or ".." in path.parts or str(path) != name
                or name == "SHA256SUMS" or name in inventory):
            raise RuntimeError("Invalid or duplicate payload checksum path")
        target = root / name
        if not target.is_file() or target.is_symlink() or digest(target) != expected:
            raise RuntimeError(f"Payload checksum mismatch: {name}")
        inventory[name] = expected
    actual = {str(path.relative_to(root)) for path in root.rglob("*") if path.is_file()}
    if not inventory or actual != set(inventory) | {"SHA256SUMS"}:
        raise RuntimeError("Payload checksum inventory is incomplete")
    return inventory


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True, help="Directory containing one archive and its .sha256")
    parser.add_argument("--revision", required=True, help="Expected complete source commit")
    parser.add_argument("--output", type=Path, required=True, help="New or empty directory under artifacts/")
    options = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("Candidate execution currently requires native GNU/Linux x86-64")
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", options.revision):
        parser.error("--revision must be a complete Git commit hash")
    output = options.output.resolve()
    if not output.is_relative_to(ROOT.joinpath("artifacts").resolve()):
        parser.error("Output must stay inside the ignored artifacts/ directory")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output must be new or empty; previous evidence is retained")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "status": "in_progress", "revision": options.revision,
              "checker_sha256": digest(Path(__file__)), "commands": [],
              "environment": {"system": platform.system(), "machine": platform.machine(),
                              "kernel": platform.release(), "os_release": platform.freedesktop_os_release(),
                              "runner_image": {key: os.environ[key] for key in
                                               ("ImageOS", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH") if key in os.environ}}}

    def save() -> None:
        output.joinpath("compatibility-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def invoke(arguments: list[str], cwd: Path, timeout: int = 30) -> str:
        process = subprocess.Popen(arguments, cwd=cwd, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
                                   env={**os.environ, "LC_ALL": "C"})
        timed_out = False
        try:
            stdout, stderr = process.communicate(input=b"", timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            stdout, stderr = process.communicate()
        record = {"arguments": arguments, "returncode": process.returncode, "timed_out": timed_out,
                  "stdout": stdout.decode("utf-8"), "stderr": stderr.decode("utf-8")}
        report["commands"].append(record)
        save()
        if timed_out or process.returncode or stderr:
            raise RuntimeError(f"Candidate validation command failed: {arguments!r}")
        return record["stdout"]

    try:
        save()
        candidates = sorted(options.input.resolve().glob("larp-*.tar.gz"))
        if len(candidates) != 1:
            raise RuntimeError("Input must contain exactly one larp-*.tar.gz candidate")
        archive = candidates[0]
        sidecar = archive.with_name(archive.name + ".sha256")
        if (not archive.is_file() or not sidecar.is_file() or archive.is_symlink()
                or sidecar.is_symlink() or archive.stat().st_size > MAX_BYTES or sidecar.stat().st_size > 4096):
            raise RuntimeError("Candidate archive/sidecar must be regular local files within the size limit")
        archive_hash = digest(archive)
        if sidecar.read_text(encoding="utf-8") != f"{archive_hash}  {archive.name}\n":
            raise RuntimeError("Candidate archive checksum does not match its sidecar")
        report.update({"archive": archive.name, "archive_sha256": archive_hash})
        expected_root = archive.name.removesuffix(".tar.gz")
        with tarfile.open(archive, "r:gz") as source:
            members = []
            names = set()
            expanded = 0
            for member in source:
                expanded += member.size
                if len(members) >= 8192 or expanded > MAX_BYTES:
                    raise RuntimeError("Candidate payload exceeds validation limits")
                path = PurePosixPath(member.name)
                if (path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != expected_root
                        or str(path) != member.name.rstrip("/") or any(char in member.name for char in "\n\r\\")
                        or (len(path.parts) > 1 and path.parts[1] in PRIVATE)
                        or member.name in names or not (member.isfile() or member.isdir())
                        or member.mode & 0o7777 not in {0o644, 0o755}):
                    raise RuntimeError(f"Unsupported candidate archive entry: {member.name!r}")
                names.add(member.name)
                members.append(member)
            unpacked = output / "unpacked"
            unpacked.mkdir()
            source.extractall(unpacked, members=members, filter="data")
        root = unpacked / expected_root
        contents = checked_contents(root)
        metadata = json.loads(root.joinpath("BUILD.json").read_text(encoding="utf-8"))
        package = tomllib.loads(root.joinpath("Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]
        if (not isinstance(metadata, dict) or metadata.get("schema_version") != 1 or metadata.get("project") != "LARP"
                or metadata.get("revision") != options.revision
                or metadata.get("target") != "x86_64-unknown-linux-gnu" or metadata.get("license") != "MIT"
                or metadata.get("version") != package["version"] or package.get("license") != "MIT"
                or expected_root != f"larp-{package['version']}-x86_64-unknown-linux-gnu-{options.revision[:12]}"
                or not root.joinpath("LICENSE").read_text(encoding="utf-8").startswith("MIT License\n")):
            raise RuntimeError("Candidate provenance does not match the expected source/version/target/license")
        compiler = metadata.get("rustc", "")
        if (not isinstance(compiler, str) or not compiler.startswith("rustc 1.99.0 ")
                or "release: 1.99.0" not in compiler.splitlines()
                or "host: x86_64-unknown-linux-gnu" not in compiler.splitlines()):
            raise RuntimeError("Preview compatibility requires the recorded Rust 1.99.0 GNU/Linux build toolchain")
        notices = metadata.get("rust_runtime_notices")
        if not isinstance(notices, list) or not notices or "THIRD_PARTY.md" not in contents:
            raise RuntimeError("Candidate is missing its Rust runtime notices")
        notice_paths = set()
        for notice in notices:
            if (not isinstance(notice, dict) or not isinstance(notice.get("path"), str)
                    or not notice["path"].startswith("THIRD_PARTY/rust/")
                    or notice["path"] not in contents or notice["path"] in notice_paths
                    or contents.get(notice["path"]) != notice.get("sha256")):
                raise RuntimeError("Runtime notice inventory is inconsistent")
            notice_paths.add(notice["path"])
        if notice_paths != {name for name in contents if name.startswith("THIRD_PARTY/rust/")}:
            raise RuntimeError("Runtime notice inventory is incomplete")
        executable = root / "bin/larp"
        with executable.open("rb") as stream:
            header = stream.read(20)
        if (executable.stat().st_mode & 0o777 != 0o755 or not header.startswith(b"\x7fELF\x02\x01")
                or header[18:20] != b"\x3e\x00" or contents.get("bin/larp") != metadata.get("binary_sha256")):
            raise RuntimeError("Candidate CLI identity, architecture or executable mode is invalid")
        report.update({"version": package["version"], "target": metadata["target"],
                       "rustc": compiler, "cargo": metadata.get("cargo"),
                       "binary_sha256": contents["bin/larp"], "checksummed_files": len(contents)})
        report["environment"]["glibc"] = invoke(["getconf", "GNU_LIBC_VERSION"], root).strip()
        abi = invoke(["readelf", "--program-headers", "--dynamic", "--version-info", str(executable)], root)
        report["abi"] = {"glibc_symbol_versions": sorted(set(re.findall(r"\bGLIBC_[A-Za-z0-9_.]+\b", abi))),
                         "needed": re.findall(r"\(NEEDED\).*?\[(.*?)\]", abi)}
        invoke(["sha256sum", "-c", "SHA256SUMS"], root)
        version = invoke([str(executable), "--version"], root)
        if not version.startswith(f"larp {package['version']} "):
            raise RuntimeError("Candidate CLI reports an unexpected version")
        identity = json.loads(invoke([str(executable), "--version", "--json"], root))
        validate_identity(identity, package["version"],
                          specification_version(root.joinpath("docs/spec/v0.1.md").read_text(encoding="utf-8")))
        if identity != metadata.get("toolchain_identity"):
            raise RuntimeError("Actual CLI protocol capabilities differ from the package provenance")
        report["identity"] = identity
        version_output = output / "version-contract"
        invoke([sys.executable, "-B", str(root / "tools/check_version.py"), "--bin", str(executable),
                "--output", str(version_output)], root, timeout=90)
        version_report = json.loads(version_output.joinpath("version-report.json").read_text(encoding="utf-8"))
        if (version_report.get("status") != "passed" or version_report.get("identity") != identity
                or version_report.get("binary_sha256") != contents["bin/larp"]
                or version_report.get("preview") != metadata.get("preview")):
            raise RuntimeError("Packaged version/historical artifact contract did not pass")
        report["historical_versions"] = version_report["historical_versions"]
        report["preview"] = version_report.get("preview")
        manifest = json.loads(root.joinpath("tests/conformance/manifest.json").read_text(encoding="utf-8"))
        if (not isinstance(manifest, dict) or manifest.get("schema_version") != 3
                or not isinstance(manifest.get("cases"), list)
                or any(not isinstance(case, dict) or case.get("status") != "active" for case in manifest["cases"])):
            raise RuntimeError("Preview compatibility requires the complete active span-aware corpus")
        cases = len(manifest["cases"])
        invoke([sys.executable, str(root / "tools/check_corpus.py")], root)
        report["conformance_modes"] = []
        for flags in ([], ["--gc-stress"], ["--artifacts", "--gc-stress"],
                      ["--formatted", "--artifacts", "--gc-stress"]):
            result = invoke([sys.executable, str(root / "tools/run_conformance.py"), "--bin", str(executable),
                             *flags], root, timeout=180)
            if not cases or not result.endswith(f"{cases}/{cases} executed cases passed; 0 cases remain pending.\n"):
                raise RuntimeError("Packaged conformance corpus did not execute every declared case")
            report["conformance_modes"].append({"flags": flags, "passed": cases, "pending": 0})
            save()
        project_output = output / "projects"
        invoke([sys.executable, "-B", str(root / "tools/run_projects.py"), "--bin", str(executable),
                "--output", str(project_output)], root, timeout=120)
        projects = json.loads(project_output.joinpath("project-report.json").read_text(encoding="utf-8"))
        project_summary = {key: projects[key] for key in
                           ("status", "binary_sha256", "manifest_sha256", "project_count", "vm_executions")}
        project_summary["cases"] = [{key: case[key] for key in ("id", "status", "modes")}
                                    for case in projects["cases"]]
        validate_projects(project_summary, contents["bin/larp"])
        inventory = json.loads(root.joinpath("tests/projects/manifest.json").read_text(encoding="utf-8"))
        if (projects.get("schema_version") != 1
                or projects["manifest_sha256"] != contents["tests/projects/manifest.json"]
                or [case["id"] for case in projects["cases"]] != [case["id"] for case in inventory["cases"]]):
            raise RuntimeError("Packaged project execution differs from the archived fixture inventory")
        report["projects"] = project_summary
        workflow_output = output / "project-workflow"
        invoke([sys.executable, "-B", str(root / "tools/check_project_workflow.py"), "--bin", str(executable),
                "--output", str(workflow_output)], output, timeout=120)
        workflow = json.loads(workflow_output.joinpath("workflow-report.json").read_text(encoding="utf-8"))
        portable_workflow = workflow_summary(workflow)
        validate_workflow(portable_workflow, contents["bin/larp"])
        if workflow["driver_sha256"] != contents["tools/check_project_workflow.py"]:
            raise RuntimeError("Project workflow did not execute the archived validation driver")
        report["project_workflow"] = portable_workflow
        save()
        if checked_contents(root) != contents:
            raise RuntimeError("Candidate payload changed during execution")
        report.update({"status": "passed", "conformance_cases": cases, "payload_unchanged": True})
        save()
        print(f"Verified existing candidate {archive.name}: {cases} cases; SHA-256 {archive_hash}")
        print(f"Host: {report['environment']['os_release'].get('PRETTY_NAME', 'Linux')}; {report['environment']['glibc']}")
    except (OSError, ValueError, TypeError, KeyError, AttributeError, RuntimeError, tarfile.TarError) as error:
        report.update({"status": "failed", "error": str(error)})
        save()
        raise


if __name__ == "__main__":
    main()
