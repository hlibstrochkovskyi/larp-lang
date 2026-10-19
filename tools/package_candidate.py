#!/usr/bin/env python3
"""Package and execute a local source/binary candidate from verified build evidence."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import tarfile
import tomllib

from version_contract import preview_metadata, specification_version, validate_identity, version_channel

ROOT = Path(__file__).resolve().parents[1]
PRIVATE = {"Ideas-not-for push!", ".git", ".git-foundation", ".agents", ".codex", ".aws", "artifacts", "target"}
RESERVED = {"bin/larp", "PACKAGE.md", "BUILD.json", "SHA256SUMS", "THIRD_PARTY.md"}
MAX_BYTES = 64 * 1024 * 1024


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True, help="Passed reproducibility report.json")
    parser.add_argument("--output", type=Path, required=True, help="New or empty directory under artifacts/")
    options = parser.parse_args()
    output = options.output.resolve()
    if not output.is_relative_to(ROOT.joinpath("artifacts").resolve()):
        parser.error("Output must be inside this project's ignored artifacts/ directory")
    if output.exists() and any(output.iterdir()):
        parser.error("Output must be empty; existing evidence is never removed or overwritten")
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("This experimental candidate currently requires native x86-64 Linux")
    report_path = options.report.resolve()
    report = json.loads(report_path.read_text(encoding="utf-8"))
    if (report.get("status") != "passed" or report.get("target") != "x86_64-unknown-linux-gnu"
            or not all(report.get(key) for key in ("binary_bytes_equal", "binary_sha256_equal", "smoke_artifacts_equal"))
            or len(report.get("builds", [])) != 2 or len(report.get("smoke_commands", [])) != 10
            or any(command["returncode"] for command in report["smoke_commands"])):
        parser.error("A complete passed native Linux reproducibility report is required")
    revision = report["revision"]
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision):
        parser.error("Report must identify a complete Git commit hash")
    archive = report_path.parent / "tracked-source.tar"
    source_bytes = archive.read_bytes()
    binaries = [Path(build["binary"]).read_bytes() for build in report["builds"]]
    if (digest(source_bytes) != report["archive_sha256"] or binaries[0] != binaries[1]
            or any(digest(data) != build["binary_sha256"] for data, build in zip(binaries, report["builds"], strict=True))):
        parser.error("Source archive or binaries changed since the reproducibility check")
    if not binaries[0].startswith(b"\x7fELF\x02\x01") or binaries[0][18:20] != b"\x3e\x00":
        parser.error("Candidate binary must be a little-endian ELF64 x86-64 executable")
    files: dict[str, tuple[bytes, int]] = {}
    with tarfile.open(fileobj=io.BytesIO(source_bytes)) as source:
        for member in source.getmembers():
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] in PRIVATE
                    or any(character in member.name for character in "\n\r\\")):
                parser.error(f"Unsupported source archive path: {member.name!r}")
            if member.isdir():
                continue
            if not member.isfile() or str(path) in files or str(path) in RESERVED or path.parts[0] == "THIRD_PARTY":
                parser.error(f"Unsupported, duplicate, or reserved source archive entry: {member.name!r}")
            if member.size > MAX_BYTES or sum(len(data) for data, _ in files.values()) + member.size > MAX_BYTES:
                parser.error("Candidate source contents exceed the 64 MiB packaging limit")
            stream = source.extractfile(member)
            assert stream is not None
            files[str(path)] = (stream.read(), 0o755 if member.mode & 0o111 else 0o644)
    package = tomllib.loads(files["Cargo.toml"][0].decode("utf-8"))["workspace"]["package"]
    version = package["version"]
    try:
        version_channel(version)
    except ValueError as error:
        parser.error(str(error))
    if package.get("license") != "MIT" or not files.get("LICENSE", (b"", 0))[0].startswith(b"MIT License\n"):
        parser.error("The archived source must contain the selected MIT license and Cargo metadata")
    epoch = int(report["controlled_environment"]["SOURCE_DATE_EPOCH"])
    name = f"larp-{version}-{report['target']}-{revision[:12]}"
    output.mkdir(parents=True, exist_ok=True)
    evidence = {"schema_version": 1, "status": "in_progress", "revision": revision,
                "packaging_driver_sha256": digest(Path(__file__).read_bytes()), "commands": []}

    def save() -> None:
        (output / "package-report.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    try:
        version_output = subprocess.run([report["builds"][0]["binary"], "--version", "--json"],
                                        input=b"", capture_output=True, timeout=15)
        if version_output.returncode or version_output.stderr:
            raise RuntimeError("Candidate CLI must support --version --json without errors")
        identity = json.loads(version_output.stdout)
        validate_identity(identity, version, specification_version(files["docs/spec/v0.1.md"][0].decode("utf-8")))
        preview = preview_metadata(identity, lambda name: files[name][0])
        compiler = report["rustc_executable"]
        current_compiler = subprocess.check_output([compiler, "-Vv"], text=True, timeout=15).strip()
        if current_compiler != report["rustc"]:
            raise RuntimeError("The report's compiler changed; runtime notices must match the original build")
        sysroot = Path(subprocess.check_output([compiler, "--print", "sysroot"], text=True, timeout=15).strip())
        notices = sysroot / "share/doc/rust"
        copyright_file, license_directory = notices / "COPYRIGHT-library.html", notices / "licenses"
        if not copyright_file.is_file() or not license_directory.is_dir():
            raise RuntimeError("The compiler distribution must provide share/doc/rust/COPYRIGHT-library.html "
                               "and licenses/. Use an official Rust toolchain with its runtime notices.")
        notice_paths = [copyright_file, *sorted(license_directory.glob("*.txt"))]
        if len(notice_paths) == 1:
            raise RuntimeError("Rust runtime license texts are missing")
        notice_manifest = []
        for path in notice_paths:
            if path.is_symlink() or not path.is_file():
                raise RuntimeError("Runtime notices must be regular compiler-distribution files")
            destination = "THIRD_PARTY/rust/" + str(path.relative_to(notices))
            data = path.read_bytes()
            files[destination] = (data, 0o644)
            notice_manifest.append({"path": destination, "sha256": digest(data)})
        third_party = ("# Rust runtime notices\n\n"
                       "The files under THIRD_PARTY/rust/ are copied unchanged from the exact Rust\n"
                       "compiler distribution used for this binary. COPYRIGHT-library.html records\n"
                       "the standard library's source and dependency notices; licenses/ retains the\n"
                       "distribution's license texts. This upstream collection may cover more code\n"
                       "than this particular executable uses. These components retain their own\n"
                       "terms. LARP's original source is licensed under LICENSE (MIT).\n")
        files["THIRD_PARTY.md"] = (third_party.encode(), 0o644)
        abi = {"inspection": "not available", "glibc_symbol_versions": []}
        if shutil.which("readelf"):
            result = subprocess.run(["readelf", "--version-info", report["builds"][0]["binary"]],
                                    capture_output=True, check=True, timeout=15, env={**os.environ, "LC_ALL": "C"})
            abi = {"inspection": "readelf --version-info", "glibc_symbol_versions":
                   sorted(set(re.findall(r"\bGLIBC_[A-Za-z0-9_.]+\b", result.stdout.decode("utf-8"))))}
        provenance = {"schema_version": 1, "project": "LARP", "version": version,
                      "revision": revision, "target": report["target"], "license": "MIT",
                      "status": "experimental candidate", "source_archive_sha256": report["archive_sha256"],
                      "binary_sha256": digest(binaries[0]), "rustc": report["rustc"], "cargo": report["cargo"],
                      "toolchain_identity": identity,
                      "preview": preview,
                      "source_date_epoch": epoch, "abi": abi,
                      "rust_runtime_notices": notice_manifest,
                      "reproducibility": "Both clean build binaries and both candidate archives matched within this run"}
        files["BUILD.json"] = ((json.dumps(provenance, indent=2, sort_keys=True) + "\n").encode(), 0o644)
        platform_note = ("See [preview platforms](docs/platforms.md) for the hosted same-archive\n"
                         "execution gate; require that commit's recorded results before describing\n"
                         "its platform matrix as validated.\n") if "docs/platforms.md" in files else ""
        install = f"""# LARP {version} experimental candidate

Source commit: `{revision}`. Target: `{report['target']}`. License: [MIT](LICENSE).
Rust runtime notices are retained in [THIRD_PARTY.md](THIRD_PARTY.md).
This archive contains the tracked source snapshot, documentation, examples, and
the prebuilt CLI at `bin/larp`. It is a local build candidate, not a published release.

After extracting and entering this directory, verify and run:

```sh
sha256sum -c SHA256SUMS
./bin/larp --version
./bin/larp run examples/hello.larp
./bin/larp run examples/collections.larp --gc-stress
./bin/larp build examples/collections.larp -o collections.larpbc
./bin/larp run collections.larpbc --gc-stress
```

The prebuilt CLI does not require Rust. Building the included source requires
the toolchain described in [README.md](README.md). See the [reference](docs/reference.md)
for the implemented subset; use `./bin/larp` in place of development binary paths.
This source snapshot omits Git metadata; use the hosted repository for a development checkout.

This is a dynamically linked GNU/Linux x86-64 binary. `BUILD.json` records its
toolchain and observed GLIBC symbol-version references when readelf is available;
those observations do not establish a tested minimum distribution version.
Other operating systems, CPU architectures, and older runtime libraries are not
validated by this candidate. {platform_note}Checksums detect changes; they are not signatures.
No installer modifies system directories or shell settings.
"""
        files["PACKAGE.md"] = (install.encode(), 0o644)

        def write_candidate(binary: bytes, destination: Path) -> None:
            contents = {**files, "bin/larp": (binary, 0o755)}
            checksums = "".join(f"{digest(data)}  {path}\n" for path, (data, _) in sorted(contents.items()))
            contents["SHA256SUMS"] = (checksums.encode(), 0o644)
            directories = {name}
            for path in contents:
                directories.update(str(parent) for parent in (PurePosixPath(name) / path).parents if str(parent) != ".")
            entries = {**{directory: None for directory in directories},
                       **{name + "/" + path: value for path, value in contents.items()}}
            with destination.open("wb") as stream, gzip.GzipFile(filename="", fileobj=stream, mode="wb", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as tar:
                    for path, value in sorted(entries.items()):
                        member = tarfile.TarInfo(path)
                        member.uid = member.gid = 0
                        member.uname = member.gname = ""
                        member.mtime = epoch
                        if value is None:
                            member.type, member.mode = tarfile.DIRTYPE, 0o755
                            tar.addfile(member)
                        else:
                            data, member.mode = value
                            member.size = len(data)
                            tar.addfile(member, io.BytesIO(data))

        first, second = output / (name + ".tar.gz"), output / ("repeat-" + name + ".tar.gz")
        write_candidate(binaries[0], first)
        write_candidate(binaries[1], second)
        if first.read_bytes() != second.read_bytes():
            raise RuntimeError("Candidate archives differ; both archives are retained")
        evidence.update({"archive": str(first), "archive_sha256": digest(first.read_bytes()),
                         "archive_bytes": first.stat().st_size, "archives_equal": True})
        (output / (first.name + ".sha256")).write_text(f"{evidence['archive_sha256']}  {first.name}\n")
        unpacked = output / "unpacked"
        unpacked.mkdir()
        with tarfile.open(first) as tar:
            tar.extractall(unpacked, filter="data")
        root = unpacked / name
        for line in root.joinpath("SHA256SUMS").read_text().splitlines():
            expected, path = line.split("  ", 1)
            if digest(root.joinpath(path).read_bytes()) != expected:
                raise RuntimeError(f"Extracted content hash mismatch: {path}")
        if shutil.which("sha256sum"):
            checked = subprocess.run(["sha256sum", "-c", "SHA256SUMS"], cwd=root,
                                     capture_output=True, timeout=15, check=True)
            evidence["checksum_utility"] = {"returncode": checked.returncode,
                                            "stdout": checked.stdout.decode("utf-8"),
                                            "stderr": checked.stderr.decode("utf-8")}
        executable = root / "bin/larp"
        if executable.stat().st_mode & 0o777 != 0o755:
            raise RuntimeError("Candidate lost the CLI executable mode")
        artifact = output / "collection-smoke.larpbc"
        for arguments, stdout in [
            (["--version"], None),
            (["run", "examples/hello.larp"], "Fake it till you compile.\n"),
            (["check", "examples/collections.larp"], ""),
            (["run", "examples/collections.larp", "--gc-stress"], "5\n50\n"),
            (["build", "examples/collections.larp", "-o", str(artifact)], ""),
            (["check", str(artifact)], ""),
            (["run", str(artifact), "--gc-stress"], "5\n50\n"),
        ]:
            result = subprocess.run([str(executable), *arguments], cwd=root, input=b"", capture_output=True, timeout=15)
            record = {"arguments": arguments, "returncode": result.returncode,
                      "stdout": result.stdout.decode("utf-8"), "stderr": result.stderr.decode("utf-8")}
            evidence["commands"].append(record)
            save()
            matches = record["stdout"].startswith(f"larp {version} ") if stdout is None else record["stdout"] == stdout
            if result.returncode or not matches or record["stderr"]:
                raise RuntimeError(f"Extracted candidate failed its CLI contract: {record}")
        evidence.update({"status": "passed", "checksummed_files": len(files) + 1,
                         "extracted_hashes_verified": True, "executable_mode": "0755"})
        save()
        print(f"Verified candidate: {first}\nSHA-256: {evidence['archive_sha256']}")
    except (OSError, ValueError, KeyError, RuntimeError, tarfile.TarError, subprocess.SubprocessError) as error:
        evidence.update({"status": "failed", "error": str(error)})
        save()
        raise


if __name__ == "__main__":
    main()
