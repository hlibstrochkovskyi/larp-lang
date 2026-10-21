#!/usr/bin/env python3
"""Execute checked-in multi-file projects with one actual LARP executable."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MODES = ["source", "source-gc-stress", "artifact", "artifact-gc-stress",
         "formatted-artifact-gc-stress"]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/project-validation")
    options = parser.parse_args()
    output, binary = options.output.resolve(), options.bin.resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output must be new or empty; existing evidence is retained")
    manifest_path = ROOT / "tests/projects/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    cases = manifest.get("cases")
    if (manifest.get("schema_version") != 1 or not isinstance(cases, list) or not cases
            or any(not isinstance(case, dict) or not isinstance(case.get("id"), str)
                   or not re.fullmatch(r"[a-z][a-z0-9-]*", case["id"]) for case in cases)
            or len({case["id"] for case in cases}) != len(cases)):
        parser.error("Project inventory needs unique filename-safe case IDs")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "status": "in_progress",
              "manifest_sha256": digest(manifest_path), "binary_sha256": digest(binary),
              "cases": [], "commands": []}

    def save() -> None:
        output.joinpath("project-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def invoke(arguments, stdout="", stderr=""):
        process = subprocess.Popen([str(binary), *arguments], cwd=ROOT,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True, env={**os.environ, "LC_ALL": "C"})
        timed_out = False
        try:
            actual_out, actual_err = process.communicate(input=b"", timeout=15)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            actual_out, actual_err = process.communicate()
        actual_out, actual_err = actual_out.decode("utf-8"), actual_err.decode("utf-8")
        report["commands"].append({"arguments": arguments, "returncode": process.returncode,
                                   "timed_out": timed_out, "stdout": actual_out, "stderr": actual_err})
        save()
        if timed_out or process.returncode or actual_err != stderr or (stdout is not None and actual_out != stdout):
            raise RuntimeError(f"Project command failed its execution contract: {arguments!r}")
        return actual_out

    try:
        save()
        for case in cases:
            project = (ROOT / case["directory"]).resolve()
            if not project.is_relative_to(ROOT / "tests/projects") or not project.is_dir():
                raise ValueError("Project fixture must stay inside tests/projects")
            sources = case["sources"]
            if (not isinstance(sources, list) or not sources or len(set(sources)) != len(sources)
                    or case["entry"] not in sources
                    or not isinstance(case.get("stdout"), str) or not isinstance(case.get("stderr"), str)):
                raise ValueError("Project inventory requires sources, entry, stdout, and stderr")
            destination = output / case["id"]
            original, formatted = destination / "source", destination / "formatted"
            fingerprints = {}
            for name in sources:
                path = PurePosixPath(name)
                source = project / name
                if (path.is_absolute() or ".." in path.parts or str(path) != name
                        or path.suffix != ".larp" or source.is_symlink() or not source.is_file()
                        or not source.resolve().is_relative_to(project)):
                    raise ValueError("Invalid project source inventory path")
                fingerprints[name] = digest(source)
                for target_root in (original, formatted):
                    target = target_root / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
            record = {"id": case["id"], "status": "in_progress", "sources": fingerprints, "modes": []}
            report["cases"].append(record)
            entry = original / case["entry"]
            invoke(["check", str(entry)])
            for mode, flags in (("source", []), ("source-gc-stress", ["--gc-stress"])):
                invoke(["run", str(entry), *flags], case["stdout"], case["stderr"])
                record["modes"].append(mode)
            artifact, formatted_artifact = destination / "original.larpbc", destination / "formatted.larpbc"
            invoke(["build", str(entry), "-o", str(artifact)])
            for name in sources:
                path = str(formatted / name)
                text = invoke(["fmt", path, "--stdout"], stdout=None)
                invoke(["fmt", path])
                invoke(["fmt", path, "--check"])
                invoke(["fmt", path, "--stdout"], stdout=text)
            invoke(["build", str(formatted / case["entry"]), "-o", str(formatted_artifact)])
            # Retain snapshots while making the original embedded paths unavailable.
            original.rename(destination / "source-snapshot")
            formatted.rename(destination / "formatted-snapshot")
            for mode, path, flags in (("artifact", artifact, []),
                                       ("artifact-gc-stress", artifact, ["--gc-stress"]),
                                       ("formatted-artifact-gc-stress", formatted_artifact, ["--gc-stress"])):
                invoke(["check", str(path)])
                invoke(["run", str(path), *flags], case["stdout"], case["stderr"])
                record["modes"].append(mode)
            if record["modes"] != MODES:
                raise RuntimeError("Project execution mode inventory is incomplete")
            record["status"] = "passed"
            save()
            print("PASS " + case["id"], flush=True)
        report.update({"status": "passed", "project_count": len(cases),
                       "vm_executions": sum(command["arguments"][0] == "run" for command in report["commands"])})
        save()
        print(f"Validated {len(cases)} projects, {report['vm_executions']} VM executions, "
              f"{len(report['commands'])} CLI commands.")
    except (OSError, ValueError, TypeError, KeyError, RuntimeError) as error:
        report.update({"status": "failed", "error": str(error)})
        save()
        raise


if __name__ == "__main__":
    main()
