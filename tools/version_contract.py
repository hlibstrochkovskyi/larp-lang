"""Shared validation of toolchain identity, separately from build provenance."""

import hashlib
import json
import posixpath
import re
from collections.abc import Callable

SEMVER = re.compile(
    r"(?P<major>0|[1-9][0-9]*)\.(?P<minor>0|[1-9][0-9]*)\.(?P<patch>0|[1-9][0-9]*)"
    r"(?:-(?P<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+(?P<metadata>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def version_channel(version: str) -> str:
    match = SEMVER.fullmatch(version)
    if match is None:
        raise ValueError("Toolchain version must use SemVer 2.0.0")
    prerelease = match["prerelease"]
    if prerelease is None:
        return "release"
    if any(part.isdigit() and len(part) > 1 and part[0] == "0" for part in prerelease.split(".")):
        raise ValueError("Numeric prerelease identifiers cannot have leading zeroes")
    if prerelease == "dev" or prerelease.startswith("dev."):
        return "development"
    return "preview" if prerelease.startswith("preview.") else "prerelease"


def validate_identity(identity: dict, version: str, specification: str) -> None:
    if (not isinstance(identity, dict) or type(identity.get("schema_version")) is not int
            or identity["schema_version"] != 1 or identity.get("name") != "larp"
            or identity.get("version") != version or identity.get("channel") != version_channel(version)
            or identity.get("language_spec") != specification or identity.get("language_status") != "experimental"
            or type(identity.get("diagnostics_schema")) is not int or identity["diagnostics_schema"] != 1):
        raise ValueError("CLI identity does not match the version/specification/protocol contract")
    artifact = identity.get("artifact")
    if not isinstance(artifact, dict):
        raise ValueError("CLI identity is missing artifact capabilities")
    readable = artifact.get("read")
    emitted = artifact.get("write")
    if (not isinstance(readable, list) or not readable
            or any(type(value) is not int or not 1 <= value <= 65535 for value in readable)
            or readable != sorted(set(readable)) or type(emitted) is not int or emitted not in readable):
        raise ValueError("CLI artifact read/write capabilities are inconsistent")


def specification_version(text: str) -> str:
    match = re.match(r"# LARP Language Specification v([0-9]+\.[0-9]+)\n", text)
    if match is None:
        raise ValueError("Source snapshot must identify its language specification baseline")
    return match[1]


def _preview_descriptor(identity: dict) -> dict | None:
    if identity["channel"] != "preview":
        return None
    version = identity["version"]
    if re.fullmatch(r"0\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)-preview\.[1-9][0-9]*", version) is None:
        raise ValueError("Named previews require 0.y.z-preview.N with positive N and no build metadata")
    stem = f"docs/releases/{version}"
    return {"schema_version": 1, "version": version, "proposed_tag": "v" + version,
            "notes": stem + ".md", "language_spec": identity["language_spec"],
            "target": "x86_64-unknown-linux-gnu", "execution_hosts": ["ubuntu-24.04", "ubuntu-26.04"],
            "artifact": identity["artifact"], "diagnostics_schema": identity["diagnostics_schema"],
            "identity_schema": identity["schema_version"]}


def validate_preview_evidence(identity: dict, evidence: dict | None) -> None:
    expected = _preview_descriptor(identity)
    if expected is None:
        if evidence is not None:
            raise ValueError("A non-preview identity cannot claim named-preview evidence")
        return
    if not isinstance(evidence, dict) or any(key not in evidence for key in expected):
        raise ValueError("Named preview is missing its release-document evidence")
    actual = {key: evidence[key] for key in expected}
    if json.dumps(actual, sort_keys=True) != json.dumps(expected, sort_keys=True):
        raise ValueError("Preview evidence does not match the declared identity")
    for key in ("descriptor_sha256", "notes_sha256"):
        if not isinstance(evidence.get(key), str) or not re.fullmatch(r"[0-9a-f]{64}", evidence[key]):
            raise ValueError("Preview evidence is missing a document SHA-256")
    links = evidence.get("relative_file_links_checked")
    if type(links) is not int or links < 0:
        raise ValueError("Preview evidence is missing its relative-link check count")


def preview_metadata(identity: dict, read_file: Callable[[str], bytes]) -> dict | None:
    """Bind a named preview to source documents; this never approves or publishes it."""
    expected = _preview_descriptor(identity)
    if expected is None:
        return None
    descriptor_bytes = read_file(expected["notes"].removesuffix(".md") + ".json")
    descriptor = json.loads(descriptor_bytes)
    # Canonical JSON distinguishes booleans from numeric schema/version values.
    if json.dumps(descriptor, sort_keys=True) != json.dumps(expected, sort_keys=True):
        raise ValueError("Preview descriptor disagrees with the actual toolchain/platform contract")
    notes_bytes = read_file(expected["notes"])
    notes = notes_bytes.decode("utf-8")
    heading = notes.split("\n", 1)[0]
    if re.fullmatch(r"# LARP " + re.escape(identity["version"]) + r"(?: — .+)?", heading) is None:
        raise ValueError("Release notes do not identify the exact preview version")
    links = 0
    for reference in re.findall(r"\[[^\]]+\]\(([^)]+)\)", notes):
        if reference.startswith(("https://", "http://", "#")):
            continue
        path = posixpath.normpath(posixpath.join("docs/releases", reference.split("#", 1)[0]))
        if path.startswith(("/", "../")) or path in {".", ".."}:
            raise ValueError("Release-note link escapes the source snapshot")
        read_file(path)
        links += 1
    return {**descriptor, "descriptor_sha256": hashlib.sha256(descriptor_bytes).hexdigest(),
            "notes_sha256": hashlib.sha256(notes_bytes).hexdigest(), "relative_file_links_checked": links}
