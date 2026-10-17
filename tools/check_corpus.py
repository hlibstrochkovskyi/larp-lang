#!/usr/bin/env python3
"""Validate the pending conformance inventory; never execute or validate LARP."""

import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "tests" / "conformance"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def main() -> None:
    manifest = json.loads((CORPUS / "manifest.json").read_text(encoding="utf-8"))
    require(set(manifest) == {"schema_version", "cases"}, "Invalid manifest keys")
    require(type(manifest["schema_version"]) is int and manifest["schema_version"] == 1,
            "Unsupported schema version")
    cases = manifest["cases"]
    require(isinstance(cases, list) and bool(cases), "No corpus cases")
    registry = (ROOT / "crates/larp-diagnostics/src/lib.rs").read_text(encoding="utf-8")
    codes = set(re.findall(r'id: "(LARP-[A-Z][0-9]{3})"', registry))
    seen_ids: set[str] = set()
    seen_paths: set[Path] = set()
    for case in cases:
        require(isinstance(case, dict), "Case must be an object")
        require(set(case) == {"id", "path", "milestone", "status", "expect"},
                "Invalid case keys")
        case_id = case["id"]
        require(isinstance(case_id, str) and re.fullmatch(r"[a-z][a-z0-9-]*", case_id)
                is not None, "Invalid case ID")
        require(case_id not in seen_ids, f"Duplicate case ID: {case_id}")
        seen_ids.add(case_id)
        require(case["status"] == "pending", f"No compiler harness exists: {case_id}")
        require(case["milestone"] in {f"M{i}" for i in range(1, 7)},
                f"Unknown milestone: {case_id}")
        require(isinstance(case["path"], str), f"Invalid path: {case_id}")
        relative = Path(case["path"])
        path = (CORPUS / relative).resolve()
        require(not relative.is_absolute() and ".." not in relative.parts,
                f"Unsafe path: {case_id}")
        require(path.is_relative_to(CORPUS.resolve()) and path.suffix == ".larp",
                f"Invalid fixture path: {case_id}")
        require(path not in seen_paths, f"Duplicate fixture path: {case_id}")
        seen_paths.add(path)
        require(path.is_file() and bool(path.read_text(encoding="utf-8").strip()),
                f"Missing or empty fixture: {case_id}")
        expect = case["expect"]
        require(isinstance(expect, dict), f"Invalid expectation: {case_id}")
        kind = expect.get("kind")
        if kind == "run":
            require(set(expect) == {"kind", "stdout"}, f"Invalid run fields: {case_id}")
            require(isinstance(expect["stdout"], str), f"Invalid stdout: {case_id}")
        else:
            require(kind in {"compile_error", "runtime_error"},
                    f"Unknown expectation kind: {case_id}")
            require(set(expect) == {"kind", "code"}, f"Invalid error fields: {case_id}")
            require(expect["code"] in codes, f"Unknown diagnostic: {case_id}")
            require((kind == "runtime_error") == expect["code"].startswith("LARP-R"),
                    f"Wrong diagnostic family: {case_id}")
    actual = {path.resolve() for path in CORPUS.rglob("*.larp")}
    require(actual == seen_paths, f"Unlisted fixtures: {sorted(actual - seen_paths)}")
    print(f"Validated {len(cases)} pending fixture definitions. "
          "No LARP programs were compiled or executed.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        sys.exit(f"Corpus inventory error: {error}")
