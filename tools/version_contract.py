"""Shared validation of toolchain identity, separately from build provenance."""

import re

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
