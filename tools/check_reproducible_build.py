#!/usr/bin/env python3
"""Compare two clean local CLI release builds of one tracked Git commit."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import shutil
import subprocess
import sys
import tarfile
import time
import uuid


ROOT = Path(__file__).resolve().parent.parent
PRIVATE = {"Ideas-not-for push!", ".git", ".git-foundation", ".agents", ".codex", ".aws", "artifacts", "target"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def same_bytes(left: Path, right: Path) -> bool:
    with left.open("rb") as a, right.open("rb") as b:
        while True:
            first, second = a.read(1024 * 1024), b.read(1024 * 1024)
            if first != second:
                return False
            if not first:
                return True


def capture(command: list[str], **options) -> str:
    return subprocess.check_output(command, text=True, **options).strip()


def build_process(command: list[str], **options) -> int:
    """Bound the build process group, including compiler/linker descendants."""
    process = subprocess.Popen(command, start_new_session=True, **options)
    try:
        return process.wait(timeout=600)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD", help="Tracked commit to archive (never the working tree)")
    parser.add_argument("--git-dir", type=Path, help="Explicit Git metadata directory; defaults to the project's verified local wrapper or normal clone")
    parser.add_argument("--output", type=Path, help="New or empty directory inside ignored artifacts/")
    parser.add_argument("--jobs", type=int, default=2)
    options = parser.parse_args()
    if sys.platform != "linux" or platform.machine() != "x86_64":
        parser.error("This baseline currently requires local x86-64 Linux")
    if options.jobs < 1:
        parser.error("--jobs must be positive")
    if options.git_dir is not None:
        metadata = options.git_dir.resolve()
    elif (ROOT / ".git-foundation").is_dir():
        metadata = Path(capture(["sh", str(ROOT / "tools/git-local.sh"), "rev-parse", "--absolute-git-dir"]))
    else:
        metadata = Path(capture(["git", "-C", str(ROOT), "rev-parse", "--absolute-git-dir"]))
    git = ["git", f"--git-dir={metadata}", f"--work-tree={ROOT}"]
    revision = capture([*git, "rev-parse", "--verify", f"{options.revision}^{{commit}}"])
    epoch = capture([*git, "show", "-s", "--format=%ct", revision])
    tracked = subprocess.check_output([*git, "ls-tree", "-rz", "--name-only", revision]).split(b"\0")
    for raw in tracked:
        if raw and raw.decode().split("/", 1)[0] in PRIVATE:
            parser.error("Refusing to archive a commit containing private/generated root paths")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = (options.output or ROOT / "artifacts/reproducible-builds" / f"{stamp}-{uuid.uuid4().hex[:8]}").resolve()
    if not output.is_relative_to((ROOT / "artifacts").resolve()):
        parser.error("--output must stay inside this project's ignored artifacts/ directory")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output directory is not empty; existing results are never removed or overwritten")
    output.mkdir(parents=True, exist_ok=True)
    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    if cargo is None or rustc is None:
        parser.error("cargo and rustc are required")
    rust_version = capture([rustc, "-Vv"])
    host = next(line.removeprefix("host: ") for line in rust_version.splitlines() if line.startswith("host: "))
    if host != "x86_64-unknown-linux-gnu":
        parser.error("This baseline currently measures the native x86_64-unknown-linux-gnu target")
    environment = dict(os.environ)
    removed = []
    for key in list(environment):
        if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_INCREMENTAL"} or key.startswith(("CARGO_BUILD_", "CARGO_PROFILE_", "CARGO_TARGET_")):
            removed.append(key)
            environment.pop(key)
    cargo_home = output / "cargo-home"
    cargo_home.mkdir()
    environment.update({
        "CARGO_HOME": str(cargo_home), "RUSTC": rustc,
        "RUSTC_WRAPPER": "", "RUSTC_WORKSPACE_WRAPPER": "",
        "CARGO_INCREMENTAL": "0", "CARGO_NET_OFFLINE": "true",
        "CARGO_CACHE_AUTO_CLEAN_FREQUENCY": "never",
        "SOURCE_DATE_EPOCH": epoch, "TZ": "UTC", "LC_ALL": "C",
    })
    report = {
        "schema_version": 1, "status": "in_progress", "revision": revision,
        "git_metadata": str(metadata), "target": host, "platform": platform.platform(),
        "rustc": rust_version, "cargo": capture([cargo, "-Vv"]),
        "cargo_executable": cargo, "rustc_executable": rustc,
        "driver_sha256": sha256(Path(__file__)), "removed_environment_keys": sorted(removed),
        "controlled_environment": {key: environment[key] for key in (
            "CARGO_HOME", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE", "CARGO_CACHE_AUTO_CLEAN_FREQUENCY",
            "SOURCE_DATE_EPOCH", "TZ", "LC_ALL")},
        "builds": [], "smoke_commands": [],
    }

    def save() -> None:
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    try:
        archive = output / "tracked-source.tar"
        with archive.open("wb") as stream:
            subprocess.run([*git, "archive", "--format=tar", revision], stdout=stream, check=True)
        report["archive_sha256"] = sha256(archive)
        binaries = []
        for name in ("first clean build", "second independent build"):
            directory = output / name
            source, target = directory / "source", directory / "target"
            source.mkdir(parents=True)
            with tarfile.open(archive) as stream:
                for member in stream.getmembers():
                    if not member.isfile() and not member.isdir():
                        raise RuntimeError(f"Unsupported archived file kind: {member.name}")
                stream.extractall(source, filter="data")
            flags = [f"--remap-path-prefix={source}=/larp/source", f"--remap-path-prefix={target}=/larp/target"]
            local_environment = {**environment, "CARGO_TARGET_DIR": str(target),
                                 "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(flags)}
            command = [cargo, "build", "-p", "larp-cli", "--bin", "larp", "--release", "--locked", "--offline", "--target", host, "--jobs", str(options.jobs)]
            record = {"source": str(source), "target": str(target), "command": command,
                      "rustflags": flags, "cargo_encoded_rustflags": local_environment["CARGO_ENCODED_RUSTFLAGS"]}
            report["builds"].append(record)
            save()
            start = time.monotonic_ns()
            with (directory / "build.stdout.txt").open("w") as stdout, (directory / "build.stderr.txt").open("w") as stderr:
                returncode = build_process(command, cwd=source, env=local_environment, stdout=stdout, stderr=stderr)
            record.update({"elapsed_ns": time.monotonic_ns() - start, "returncode": returncode})
            if returncode:
                raise RuntimeError(f"Build failed; inspect {directory / 'build.stderr.txt'}")
            binary = target / host / "release/larp"
            record.update({"binary": str(binary), "binary_sha256": sha256(binary), "binary_bytes": binary.stat().st_size})
            binaries.append(binary)
            save()
        identical = same_bytes(*binaries)
        report["binary_bytes_equal"] = identical
        report["binary_sha256_equal"] = report["builds"][0]["binary_sha256"] == report["builds"][1]["binary_sha256"]
        report["embedded_source_or_target_paths"] = [
            {"binary": index, "path": str(path)}
            for index, binary in enumerate(binaries, 1)
            for build in report["builds"]
            for path in (build["source"], build["target"])
            if str(path).encode() in binary.read_bytes()
        ]
        if not identical or not report["binary_sha256_equal"]:
            raise RuntimeError("Release binary mismatch; both binaries and complete build trees are preserved")
        smoke = output / "smoke.larp"
        smoke.write_text('main_character { mfs<mf> values = [2, 4, 49]; values.filter(x => x mogg 3).map(x => x + 1).forEach(x => yap(x)); }\n')
        for index, binary in enumerate(binaries, 1):
            artifact = output / f"smoke-{index}.larpbc"
            for arguments, expected in [
                (["check", str(smoke)], ""),
                (["run", str(smoke), "--gc-stress"], "5\n50\n"),
                (["build", str(smoke), "-o", str(artifact)], ""),
                (["check", str(artifact)], ""),
                (["run", str(artifact), "--gc-stress"], "5\n50\n"),
            ]:
                completed = subprocess.run([str(binary), *arguments], input="", text=True, capture_output=True, timeout=15)
                record = {"binary": index, "arguments": arguments, "returncode": completed.returncode,
                          "stdout": completed.stdout, "stderr": completed.stderr}
                report["smoke_commands"].append(record)
                save()
                if completed.returncode or completed.stdout != expected or completed.stderr:
                    raise RuntimeError(f"Smoke check failed: {record}")
        report["smoke_artifacts_equal"] = same_bytes(output / "smoke-1.larpbc", output / "smoke-2.larpbc")
        if not report["smoke_artifacts_equal"]:
            raise RuntimeError("Smoke bytecode artifacts differ")
        report["status"] = "passed"
        save()
        print(f"Two clean release builds matched byte-for-byte: {report['builds'][0]['binary_sha256']}")
        print(f"Revision: {revision}\nEvidence: {output / 'report.json'}")
        return 0
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        report.update({"status": "failed", "error": str(error)})
        save()
        print(f"Reproducible build check failed: {error}\nEvidence: {output / 'report.json'}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
