#!/usr/bin/env python3
"""Execute every complete LARP fence in the practical reference through the CLI."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/reference-validation")
    options = parser.parse_args()
    output = options.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error("Output must be empty; choose a fresh --output to retain previous evidence")
    document = ROOT / "docs/reference.md"
    contracts = json.loads(ROOT.joinpath("docs/reference-cases.json").read_text(encoding="utf-8"))
    if contracts.get("schema_version") != 1:
        parser.error("Unsupported reference contract schema")
    cases = contracts["cases"]
    blocks = re.findall(r"^```larp\n(.*?)^```[ \t]*$", document.read_text(encoding="utf-8"), re.M | re.S)
    if len(blocks) != len(cases) or not blocks:
        parser.error("Every LARP code fence must have exactly one ordered execution contract")
    identifiers = [case["id"] for case in cases]
    if len(set(identifiers)) != len(cases) or any(not re.fullmatch(r"[a-z][a-z0-9-]*", name) for name in identifiers):
        parser.error("Reference case IDs must be unique lowercase filename-safe names")
    subprocess.run(["cargo", "build", "-p", "larp-cli", "--locked", "--offline"], cwd=ROOT, check=True)
    binary = ROOT / "target/debug/larp"
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "samples": len(blocks),
              "reference_sha256": hashlib.sha256(document.read_bytes()).hexdigest(),
              "contracts_sha256": hashlib.sha256(ROOT.joinpath("docs/reference-cases.json").read_bytes()).hexdigest(),
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "commands": []}

    def invoke(arguments: list[str], stdin: str = "", stdout: str | None = "", stderr: str = "") -> str:
        result = subprocess.run([str(binary), *arguments], input=stdin.encode("utf-8"),
                                capture_output=True, cwd=ROOT, timeout=10)
        actual_out, actual_err = result.stdout.decode("utf-8"), result.stderr.decode("utf-8")
        report["commands"].append({"arguments": arguments, "stdin": stdin,
                                   "returncode": result.returncode,
                                   "stdout": actual_out, "stderr": actual_err})
        (output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        if result.returncode != 0 or actual_err != stderr or (stdout is not None and actual_out != stdout):
            raise RuntimeError(f"Reference command failed its contract: {arguments!r}; "
                               f"exit={result.returncode}, stdout={actual_out!r}, stderr={actual_err!r}")
        return actual_out

    for code, case in zip(blocks, cases, strict=True):
        source, formatted, artifact = [output / (case["id"] + suffix)
                                       for suffix in (".larp", "-formatted.larp", ".larpbc")]
        source.write_text(code, encoding="utf-8")
        program_input = case.get("input", {})
        stdin = program_input.get("stdin", "")
        arguments = ["--", *program_input["args"]] if "args" in program_input else []

        def execute(path: Path, stress: bool) -> None:
            invoke(["run", str(path), *(["--gc-stress"] if stress else []), *arguments],
                   stdin=stdin, stdout=case["stdout"], stderr=case.get("stderr", ""))

        invoke(["check", str(source)])
        execute(source, False)
        execute(source, True)
        text = invoke(["fmt", str(source), "--stdout"], stdout=None)
        formatted.write_text(text, encoding="utf-8")
        invoke(["fmt", str(formatted), "--check"])
        invoke(["fmt", str(formatted), "--stdout"], stdout=text)
        execute(formatted, True)
        invoke(["build", str(source), "-o", str(artifact)])
        invoke(["check", str(artifact)])
        execute(artifact, False)
        execute(artifact, True)
        invoke(["dump", str(artifact)], stdout=None)
        invoke(["dump", str(source), "--ir"], stdout=None)
        invoke(["fmt", str(formatted)])
        print("PASS " + case["id"], flush=True)
    invoke(["explain", "LARP-T041"], stdout=None)
    executions = sum(command["arguments"][0] == "run" for command in report["commands"])
    print(f"Validated {len(blocks)} reference samples, {executions} VM executions, "
          f"{len(report['commands'])} CLI commands. Evidence: {output}")


if __name__ == "__main__":
    main()
