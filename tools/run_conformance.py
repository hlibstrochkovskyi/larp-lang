#!/usr/bin/env python3
"""Execute declared conformance cases through the real LARP CLI and VM."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "tests" / "conformance"


def invoke(binary: Path, command: str, path: Path, gc_stress: bool = False) -> subprocess.CompletedProcess[str]:
    # Decode explicitly: text=True would normalize CR/CRLF and hide stdout errors.
    options = ["--gc-stress"] if command == "run" and gc_stress else []
    result = subprocess.run([str(binary), command, str(path), *options], capture_output=True, timeout=10)
    return subprocess.CompletedProcess(result.args, result.returncode,
                                       result.stdout.decode("utf-8"), result.stderr.decode("utf-8"))


def artifact_errors(binary: Path, source: Path, expected: dict, source_run: subprocess.CompletedProcess[str], gc_stress: bool) -> list[str]:
    """Compare the loaded-file path with real compilation, including failures."""
    with tempfile.TemporaryDirectory(prefix="larp-conformance-") as directory:
        artifact = Path(directory) / "program.larpbc"
        built = subprocess.run([str(binary), "build", str(source), "-o", str(artifact)],
                               capture_output=True, timeout=10)
        if expected["kind"] == "compile_error":
            match = re.search(rb"error\[(LARP-[A-Z][0-9]{3})\]", built.stderr)
            if (built.returncode != 1 or built.stdout or artifact.exists() or
                    not match or match[1].decode() != expected["code"]):
                return [f"build must reject source without an artifact: {built.stderr!r}"]
            return []
        if built.returncode != 0 or built.stdout or built.stderr or not artifact.is_file():
            return [f"artifact build failed or observed output: {built.stderr!r}"]
        checked = invoke(binary, "check", artifact)
        loaded = invoke(binary, "run", artifact, gc_stress)
        errors = []
        if checked.returncode != 0 or checked.stdout or checked.stderr:
            errors.append(f"artifact check failed or observed output: {checked.stderr}")
        if (loaded.returncode, loaded.stdout, loaded.stderr) != (source_run.returncode, source_run.stdout, source_run.stderr):
            errors.append(f"loaded execution differs: exit={loaded.returncode}, stdout={loaded.stdout!r}, stderr={loaded.stderr!r}")
        return errors


def case_errors(expectation: dict, checked: subprocess.CompletedProcess[str], output: subprocess.CompletedProcess[str]) -> list[str]:
    kind = expectation["kind"]
    errors = []
    if kind == "compile_error":
        for label, result in [("check", checked), ("run", output)]:
            match = re.search(r"error\[(LARP-[A-Z][0-9]{3})\]", result.stderr)
            if result.returncode != 1 or result.stdout or not match or match[1] != expectation["code"]:
                errors.append(f"{label} expected {expectation['code']}, got {result.returncode}: {result.stderr}")
    else:
        if checked.returncode != 0 or checked.stdout or checked.stderr:
            errors.append(f"check failed or observed output: {checked.stderr}")
        if kind == "run":
            if output.returncode != 0 or output.stdout != expectation["stdout"] or output.stderr != expectation.get("stderr", ""):
                errors.append(f"run mismatch: exit={output.returncode}, stdout={output.stdout!r}, stderr={output.stderr!r}")
        else:
            match = re.search(r"error\[(LARP-[A-Z][0-9]{3})\]", output.stderr)
            if output.returncode != 1 or output.stdout != expectation.get("stdout", "") or not match or match[1] != expectation["code"]:
                errors.append(f"expected runtime {expectation['code']}: {output.stderr}")
    if kind != "run":
        if not re.search(r" --> .+:[0-9]+:[0-9]+", output.stderr):
            errors.append("error is missing a source location")
    return errors


def formatted_errors(binary: Path, path: Path, expectation: dict, gc_stress: bool, artifacts: bool) -> list[str]:
    """Format a temporary copy, check idempotence, and execute the same contract."""
    formatted = subprocess.run([str(binary), "fmt", str(path), "--stdout"],
                               capture_output=True, timeout=10)
    if formatted.returncode != 0:
        match = re.search(rb"error\[(LARP-[A-Z][0-9]{3})\]", formatted.stderr)
        if (expectation["kind"] == "compile_error" and expectation["code"].startswith(("LARP-L", "LARP-P"))
                and formatted.returncode == 1 and not formatted.stdout
                and match and match[1].decode() == expectation["code"]):
            return []
        return [f"fmt failed: {formatted.stderr!r}"]
    if formatted.stderr:
        return [f"fmt observed output: {formatted.stderr!r}"]
    with tempfile.TemporaryDirectory(prefix="larp-formatted-") as directory:
        target = Path(directory) / "program.larp"
        target.write_bytes(formatted.stdout)
        stable = subprocess.run([str(binary), "fmt", str(target), "--check"],
                                capture_output=True, timeout=10)
        errors = []
        if stable.returncode or stable.stdout or stable.stderr:
            errors.append(f"formatted source is not idempotent: {stable.stderr!r}")
        checked = invoke(binary, "check", target)
        output = invoke(binary, "run", target, gc_stress)
        errors.extend(case_errors(expectation, checked, output))
        if artifacts:
            errors.extend(artifact_errors(binary, target, expectation, output, gc_stress))
        return [f"formatted: {error}" for error in errors]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, help="Use an already built larp binary")
    parser.add_argument("--all", action="store_true", help="Also attempt pending cases; failures are expected")
    parser.add_argument("--gc-stress", action="store_true", help="Collect at every object/array/closure allocation safepoint")
    parser.add_argument("--artifacts", action="store_true", help="Also build, load, verify, and execute persisted bytecode")
    parser.add_argument("--formatted", action="store_true", help="Also format temporary copies, check idempotence, and execute them")
    args = parser.parse_args()
    subprocess.run([sys.executable, str(ROOT / "tools/check_corpus.py")], cwd=ROOT, check=True)
    binary = args.bin
    if binary is None:
        subprocess.run(["cargo", "build", "-p", "larp-cli", "--locked", "--offline"], cwd=ROOT, check=True)
        metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked", "--offline"],
                                  cwd=ROOT, check=True, capture_output=True, text=True)
        binary = Path(json.loads(metadata.stdout)["target_directory"]) / "debug" / "larp"
    binary = binary.resolve()
    cases = json.loads((CORPUS / "manifest.json").read_text(encoding="utf-8"))["cases"]
    selected = [case for case in cases if args.all or case["status"] == "active"]
    if not selected:
        raise ValueError("No cases selected")
    failures = []
    for case in selected:
        path = CORPUS / case["path"]
        expectation = case["expect"]
        try:
            checked = invoke(binary, "check", path)
            output = invoke(binary, "run", path, args.gc_stress)
            errors = case_errors(expectation, checked, output)
            if args.artifacts:
                errors.extend(artifact_errors(binary, path, expectation, output, args.gc_stress))
            if args.formatted:
                errors.extend(formatted_errors(binary, path, expectation, args.gc_stress, args.artifacts))
            if errors:
                failures.append(f"FAIL {case['id']}: " + "\n".join(errors))
            else:
                print(f"PASS {case['id']}")
        except subprocess.TimeoutExpired:
            failures.append(f"FAIL {case['id']}: CLI timed out")
    for failure in failures:
        print(failure, file=sys.stderr)
    print(f"{len(selected) - len(failures)}/{len(selected)} executed cases passed; "
          f"{len(cases) - len(selected)} cases remain pending.")
    return int(bool(failures))


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f"Conformance harness error: {error}")
