#!/usr/bin/env python3
"""Build libFuzzer seeds from explicitly public source directories only."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MAX_INPUT = 64 * 1024
TARGETS = ("frontend", "formatter", "compiler", "artifact")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "fuzz" / "corpus")
    parser.add_argument("--binary", type=Path, default=ROOT / "target" / "debug" / "larp")
    options = parser.parse_args()
    output = options.output.resolve()
    binary = options.binary.resolve()
    if not binary.is_file():
        parser.error("Build the CLI first with cargo build --locked --offline")
    if output.exists() and any(output.iterdir()):
        parser.error("Seed output must be empty; choose a new --output for a fresh corpus")
    for target in TARGETS:
        (output / target).mkdir(parents=True, exist_ok=True)
    records = []
    cases = json.loads(ROOT.joinpath("tests/conformance/manifest.json").read_text(encoding="utf-8"))["cases"]
    expected = {"tests/conformance/" + case["path"]: case["expect"]
                for case in cases if case["status"] == "active"}

    def seed(target: str, data: bytes, origin: str) -> None:
        if len(data) > MAX_INPUT:
            return
        digest = hashlib.sha256(data).hexdigest()
        (output / target / digest).write_bytes(data)
        records.append({"target": target, "sha256": digest, "bytes": len(data), "origin": origin})

    # Never scan the workspace recursively: private design notes are not seeds.
    sources = sorted([*ROOT.joinpath("examples").rglob("*.larp"),
                      *ROOT.joinpath("tests/conformance").rglob("*.larp")])
    build_dir = output / "build"
    build_dir.mkdir(exist_ok=True)
    for path in sources:
        if path.is_symlink():
            raise RuntimeError(f"Seed sources must not be symlinks: {path}")
        relative = str(path.relative_to(ROOT))
        data = path.read_bytes()
        if len(data) > MAX_INPUT:
            continue
        for target in TARGETS[:3]:
            seed(target, data, relative)
        artifact = build_dir / (hashlib.sha256(data).hexdigest() + ".larpbc")
        built = subprocess.run([str(binary), "build", relative, "-o", str(artifact)],
                               cwd=ROOT, stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
        contract = expected.get(relative)
        must_reject = contract is not None and contract["kind"] == "compile_error"
        if must_reject:
            if (built.returncode != 1 or built.stdout or artifact.exists()
                    or ("error[" + contract["code"] + "]").encode() not in built.stderr):
                raise RuntimeError(f"Seed compilation violated the fixture contract: {relative}: {built.stderr!r}")
        elif built.returncode == 0:
            if built.stdout or built.stderr:
                raise RuntimeError(f"Artifact building unexpectedly observed output: {relative}")
            encoded = artifact.read_bytes()
            seed("artifact", encoded, relative)
            # Keep genuine complete artifacts as well as broken framing variants.
            for boundary in (0, 8, 15, 16, len(encoded) // 2, len(encoded) - 1):
                seed("artifact", encoded[:boundary], relative + f" truncated at {boundary}")
        else:
            raise RuntimeError(f"Expected a compilable public seed: {relative}: {built.stderr!r}")
    for target in TARGETS:
        for path in sorted(ROOT.joinpath("fuzz/regressions", target).glob("*")):
            if path.name in {".gitkeep", "README.md"}:
                continue
            extension = ".bin" if target == "artifact" else ".larp"
            if path.is_symlink() or not path.is_file() or path.suffix != extension:
                raise RuntimeError(f"Unexpected regression seed entry: {path}")
            seed(target, path.read_bytes(), str(path.relative_to(ROOT)))
    manifest = output / "seeds.json"
    manifest.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    counts = {target: len(list((output / target).iterdir())) for target in TARGETS}
    print(json.dumps({"corpus": str(output), "counts": counts}, sort_keys=True))


if __name__ == "__main__":
    main()
