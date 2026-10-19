#!/usr/bin/env python3
"""Check CLI identity against real diagnostics, emitted artifacts and frozen old bytes."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

from version_contract import preview_metadata, specification_version, validate_identity

ROOT = Path(__file__).resolve().parents[1]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, required=True, help="Existing CLI; never rebuilt by this check")
    parser.add_argument("--output", type=Path, required=True, help="New or empty evidence directory")
    options = parser.parse_args()
    binary, output = options.bin.resolve(), options.output.resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output must be new or empty; previous evidence is retained")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "status": "in_progress", "commands": [],
              "driver_sha256": digest(Path(__file__).read_bytes()),
              "validator_sha256": digest(Path(__file__).with_name("version_contract.py").read_bytes())}

    def save() -> None:
        output.joinpath("version-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def invoke(arguments: list[str], expected: str | None = None, code: int = 0,
               input_text: str = "", diagnostics: bool = False) -> str:
        result = subprocess.run([str(binary), *arguments], cwd=ROOT, input=input_text.encode("utf-8"),
                                capture_output=True, timeout=30)
        stdout, stderr = result.stdout.decode("utf-8"), result.stderr.decode("utf-8")
        report["commands"].append({"arguments": arguments, "returncode": result.returncode,
                                   "stdout": stdout, "stderr": stderr})
        save()
        if result.returncode != code or (expected is not None and stdout != expected) or (stderr and not diagnostics):
            raise ValueError(f"Version/compatibility command failed: {arguments!r}")
        if diagnostics:
            records = [json.loads(line) for line in stderr.split("\n") if line]
            if (len(records) != 1 or records[0].get("schema_version") != report["identity"]["diagnostics_schema"]
                    or records[0].get("kind") != "cli_error"):
                raise ValueError("Actual JSON diagnostics disagree with the reported schema")
        return stdout

    try:
        save()
        report["binary_sha256"] = digest(binary.read_bytes())
        version = tomllib.loads(ROOT.joinpath("Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
        specification = specification_version(ROOT.joinpath("docs/spec/v0.1.md").read_text(encoding="utf-8"))
        rendered = invoke(["--version", "--json"])
        identity = json.loads(rendered)
        validate_identity(identity, version, specification)
        if not rendered.endswith("\n") or rendered.count("\n") != 1:
            raise ValueError("Version JSON must be exactly one LF-terminated record")
        report["identity"] = identity
        report["preview"] = preview_metadata(identity, lambda name: ROOT.joinpath(name).read_bytes())
        ordinary = invoke(["--version"])
        if not ordinary.startswith(f"larp {version} "):
            raise ValueError("Human and machine version disagree")
        invoke(["--diagnostic-format=json", "--version", "--json"], rendered)
        invoke(["--diagnostic-format=json", "--invalid-version-contract"], "", code=2, diagnostics=True)
        source = output / "current.larp"
        source.write_text("main_character { yap(49); }\n", encoding="utf-8")
        current = output / "current.larpbc"
        invoke(["build", str(source), "-o", str(current)], "")
        contents = current.read_bytes()
        if contents[:8] != b"LARPBC\r\n" or int.from_bytes(contents[8:10], "little") != identity["artifact"]["write"]:
            raise ValueError("Actual encoder disagrees with the reported artifact write version")
        invoke(["check", str(current)], "")
        invoke(["run", str(current), "--gc-stress"], "49\n")
        fixtures = ROOT / "tests/compatibility/artifacts"
        manifest_bytes = fixtures.joinpath("manifest.json").read_bytes()
        manifest = json.loads(manifest_bytes)
        if manifest["schema_version"] != 1 or not manifest["cases"]:
            raise ValueError("Missing historical artifact inventory")
        seen = set()
        for case in manifest["cases"]:
            wire = case["wire_version"]
            if type(wire) is not int or wire in seen or wire not in identity["artifact"]["read"]:
                raise ValueError("Historical fixtures do not match the declared read versions")
            seen.add(wire)
            source_bytes = fixtures.joinpath(case["source"]).read_bytes()
            artifact = bytes.fromhex(fixtures.joinpath(case["artifact"]).read_text(encoding="ascii"))
            if (digest(source_bytes) != case["source_sha256"] or digest(artifact) != case["artifact_sha256"]
                    or artifact[:8] != b"LARPBC\r\n" or int.from_bytes(artifact[8:10], "little") != wire):
                raise ValueError("Frozen historical fixture hashes/header changed")
            target = output / f"historical-v{wire}.larpbc"
            target.write_bytes(artifact)
            invoke(["check", str(target)], "")
            for stress in ([], ["--gc-stress"]):
                invoke(["run", str(target), *stress, "--", *case["args"]], case["stdout"], input_text=case["stdin"])
        if seen != set(identity["artifact"]["read"]):
            raise ValueError("A declared artifact read version lacks an executed historical fixture")
        report.update({"status": "passed", "historical_versions": sorted(seen),
                       "fixture_manifest_sha256": digest(manifest_bytes), "emitted_version": identity["artifact"]["write"]})
        save()
        print(f"Verified {version}: artifact write {identity['artifact']['write']}, historical reads {sorted(seen)}, diagnostic schema {identity['diagnostics_schema']}")
    except (OSError, ValueError, TypeError, KeyError, AttributeError, subprocess.SubprocessError) as error:
        report.update({"status": "failed", "error": str(error)})
        save()
        raise


if __name__ == "__main__":
    main()
