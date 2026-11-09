#!/usr/bin/env python3
"""Exercise manifest projects using one existing CLI, without Rust or network access."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SCAFFOLD_OUTPUT = "Your first sidequest:\n50\nfr\n"
INPUT_OUTPUT = "first\n🦀\nhello from stdin\n50\n"
# Ordered stages are the execution contract used by the same-archive matrix gate.
STAGES = {
    "scaffold": ["new", "check", "source", "source-gc-stress", "ancestor-check",
                 "ancestor-source", "selected-check", "selected-source", "build-default",
                 "build-explicit", "artifact-check", "artifact", "artifact-gc-stress",
                 "explicit-artifact-check", "explicit-artifact-gc-stress"],
    "siblings-input": ["check", "source", "source-gc-stress", "ancestor-check",
                       "ancestor-source", "selected-check", "selected-source", "build-default",
                       "build-explicit", "artifact-check", "artifact", "artifact-gc-stress",
                       "explicit-artifact-check", "explicit-artifact-gc-stress"],
}
RUN_STAGES = {"source", "source-gc-stress", "ancestor-source", "selected-source",
              "artifact", "artifact-gc-stress", "explicit-artifact-gc-stress"}



def expected_commands(identifier: str) -> list[dict]:
    """Describe exact portable commands required by the packaged execution gate."""
    base = f"$WORK/{identifier}"
    project = base + "/app"
    manifest, nested = project + "/larp.toml", project + "/src/nested"
    default, custom = project + "/target/app.larpbc", base + "/custom.larpbc"
    arguments = [] if identifier == "scaffold" else ["--", "first", "🦀"]
    stdout = SCAFFOLD_OUTPUT if identifier == "scaffold" else INPUT_OUTPUT
    stdin = "" if identifier == "scaffold" else "hello from stdin\n"
    commands = {
        "new": (["new", "app"], base),
        "check": (["check"], project),
        "source": (["run", *arguments], project),
        "source-gc-stress": (["run", "--gc-stress", "--fuel", "10000", *arguments], project),
        "ancestor-check": (["check"], nested),
        "ancestor-source": (["run", *arguments], nested),
        "selected-check": (["check", "--manifest-path", manifest], base),
        "selected-source": (["run", "--gc-stress", "--manifest-path", manifest, *arguments], base),
        "build-default": (["build"], nested),
        "build-explicit": (["build", "-o", custom, "--manifest-path", manifest], base),
        "artifact-check": (["check", default], base),
        "artifact": (["run", default, *arguments], base),
        "artifact-gc-stress": (["run", default, "--gc-stress", *arguments], base),
        "explicit-artifact-check": (["check", custom], base),
        "explicit-artifact-gc-stress": (["run", custom, "--gc-stress", *arguments], base),
    }
    return [{"stage": stage, "arguments": commands[stage][0], "cwd": commands[stage][1],
             "stdin": stdin if stage in RUN_STAGES else "", "returncode": 0, "timed_out": False,
             "stdout": (stdout if stage in RUN_STAGES else
                        f"Created project {project}\n\n  cd app\n  larp check\n  larp run\n  larp build\n"
                        if stage == "new" else ""), "stderr": ""}
            for stage in STAGES[identifier]]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def summary(report: dict) -> dict:
    """Keep portable execution evidence; artifact source paths are host-specific."""
    result = {key: report[key] for key in
              ("schema_version", "status", "binary_sha256", "driver_sha256", "vm_executions")}
    result["cases"] = [{key: case[key] for key in
                        ("id", "status", "manifest_sha256", "sources", "sources_unavailable", "commands")}
                       for case in report["cases"]]
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New or empty evidence directory")
    options = parser.parse_args()
    output, binary = options.output.resolve(), options.bin.resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output must be new or empty; existing evidence is retained")
    if not binary.is_file():
        parser.error("--bin must identify an existing LARP executable")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "status": "in_progress", "binary_sha256": digest(binary),
              "driver_sha256": digest(Path(__file__)), "cases": []}

    def save() -> None:
        output.joinpath("workflow-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def portable(value: str) -> str:
        return value.replace(str(output), "$WORK")

    def invoke(case, stage, arguments, cwd, stdout="", stdin=""):
        process = subprocess.Popen([str(binary), *arguments], cwd=cwd, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
                                   env={**os.environ, "LC_ALL": "C"})
        timed_out = False
        try:
            actual_out, actual_err = process.communicate(input=stdin.encode("utf-8"), timeout=15)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            actual_out, actual_err = process.communicate()
        actual_out, actual_err = actual_out.decode("utf-8"), actual_err.decode("utf-8")
        case["commands"].append({"stage": stage, "arguments": [portable(arg) for arg in arguments],
                                 "cwd": portable(str(cwd)), "stdin": stdin,
                                 "returncode": process.returncode, "timed_out": timed_out,
                                 "stdout": portable(actual_out), "stderr": portable(actual_err)})
        save()
        if timed_out or process.returncode or actual_err or actual_out != stdout:
            raise RuntimeError(f"Project workflow command failed: {case['id']}/{stage}: {arguments!r}")

    try:
        save()
        for identifier in STAGES:
            case_root = output / identifier
            case_root.mkdir()
            project = case_root / "app"
            case = {"id": identifier, "status": "in_progress", "commands": []}
            report["cases"].append(case)
            if identifier == "scaffold":
                invoke(case, "new", ["new", "app"], case_root,
                       f"Created project {project}\n\n  cd app\n  larp check\n  larp run\n  larp build\n")
                expected, stdin, arguments = SCAFFOLD_OUTPUT, "", []
            else:
                (project / "src").mkdir(parents=True)
                (project / "lib").mkdir()
                project.joinpath("larp.toml").write_text(
                    'schema = 1\n\n[project]\nname = "app"\nentry = "src/main.larp"\n', encoding="utf-8")
                project.joinpath("src/main.larp").write_text(
                    'import "../lib/aura.larp" as aura;\nmain_character {\n'
                    '    args().forEach(x => yap(x));\n'
                    '    yap(readLine().cope(err => some(err)).copium("EOF"));\n'
                    '    yap(aura::earn(49));\n}\n', encoding="utf-8")
                project.joinpath("lib/aura.larp").write_text(
                    'highkey sidequest earn(mf base) -> mf uses(abort) { return base + 1; }\n', encoding="utf-8")
                expected, stdin, arguments = INPUT_OUTPUT, "hello from stdin\n", ["--", "first", "🦀"]
            manifest = project / "larp.toml"
            case["manifest_sha256"] = digest(manifest)
            case["sources"] = {str(path.relative_to(project)): digest(path)
                               for path in sorted(project.rglob("*.larp"))}
            nested = project / "src/nested"
            nested.mkdir()
            invoke(case, "check", ["check"], project)
            invoke(case, "source", ["run", *arguments], project, expected, stdin)
            invoke(case, "source-gc-stress", ["run", "--gc-stress", "--fuel", "10000", *arguments],
                   project, expected, stdin)
            invoke(case, "ancestor-check", ["check"], nested)
            invoke(case, "ancestor-source", ["run", *arguments], nested, expected, stdin)
            invoke(case, "selected-check", ["check", "--manifest-path", str(manifest)], case_root)
            invoke(case, "selected-source", ["run", "--gc-stress", "--manifest-path", str(manifest),
                                              *arguments], case_root, expected, stdin)
            default = project / "target/app.larpbc"
            custom = case_root / "custom.larpbc"
            invoke(case, "build-default", ["build"], nested)
            invoke(case, "build-explicit", ["build", "-o", str(custom), "--manifest-path", str(manifest)], case_root)
            if not default.is_file() or not custom.is_file():
                raise RuntimeError("Project builds did not create both expected artifacts")
            case["artifact_sha256"] = {"default": digest(default), "explicit": digest(custom)}
            # Preserve evidence, while removing every original source/manifest path.
            (project / "src").rename(project / "src-snapshot")
            if (project / "lib").exists():
                (project / "lib").rename(project / "lib-snapshot")
            manifest.rename(project / "larp.toml.snapshot")
            if manifest.exists() or (project / "src").exists() or (project / "lib").exists():
                raise RuntimeError("Original project sources remain accessible")
            case["sources_unavailable"] = True
            invoke(case, "artifact-check", ["check", str(default)], case_root)
            invoke(case, "artifact", ["run", str(default), *arguments], case_root, expected, stdin)
            invoke(case, "artifact-gc-stress", ["run", str(default), "--gc-stress", *arguments],
                   case_root, expected, stdin)
            invoke(case, "explicit-artifact-check", ["check", str(custom)], case_root)
            invoke(case, "explicit-artifact-gc-stress", ["run", str(custom), "--gc-stress", *arguments],
                   case_root, expected, stdin)
            if [command["stage"] for command in case["commands"]] != STAGES[identifier]:
                raise RuntimeError("Project workflow execution inventory is incomplete")
            case["status"] = "passed"
            save()
            print(f"PASS project workflow {identifier}", flush=True)
        if digest(binary) != report["binary_sha256"]:
            raise RuntimeError("Project workflow executable changed during validation")
        report.update({"status": "passed", "vm_executions": sum(
            command["arguments"][0] == "run" for case in report["cases"] for command in case["commands"])})
        save()
        print(f"Validated {len(report['cases'])} project workflows, {report['vm_executions']} VM executions, "
              f"{sum(len(case['commands']) for case in report['cases'])} CLI commands.")
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        report.update({"status": "failed", "error": str(error)})
        save()
        raise


if __name__ == "__main__":
    main()
