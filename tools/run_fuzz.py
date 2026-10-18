#!/usr/bin/env python3
"""Run bounded libFuzzer campaigns and retain logs, outcomes, and fingerprints."""

import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ("frontend", "formatter", "compiler", "artifact")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--seed", type=int, default=49)
    parser.add_argument("--rss-mib", type=int, default=1024)
    parser.add_argument("--timeout", type=int, default=5)
    parser.add_argument("--disable-leak-check", action="store_true",
                        help="Disable only LeakSanitizer when the host denies its ptrace support")
    parser.add_argument("--target", action="append", choices=TARGETS)
    parser.add_argument("--binary-dir", type=Path,
                        default=ROOT / "fuzz/target/x86_64-unknown-linux-gnu/release")
    parser.add_argument("--corpus", type=Path, default=ROOT / "fuzz/corpus")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/fuzz-campaign")
    options = parser.parse_args()
    if min(options.seconds, options.seed, options.rss_mib, options.timeout) < 1:
        parser.error("Seconds, seed, RSS, and timeout must be positive")
    output, corpus = options.output.resolve(), options.corpus.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error("Report output must be empty; choose a new --output to preserve previous evidence")
    output.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    if options.disable_leak_check:
        previous = environment.get("ASAN_OPTIONS", "")
        environment["ASAN_OPTIONS"] = previous + (":" if previous else "") + "detect_leaks=0"
    report = {"schema_version": 1, "max_input_bytes": 65536,
              "platform": platform.platform(),
              "asan_options": environment.get("ASAN_OPTIONS"),
              "lsan_options": environment.get("LSAN_OPTIONS"),
              "fuzz_lock_sha256": digest(ROOT / "fuzz/Cargo.lock"), "runs": []}
    for target in options.target or TARGETS:
        binary = options.binary_dir.resolve() / target
        seeds = corpus / target
        if not binary.is_file() or not seeds.is_dir() or not any(seeds.iterdir()):
            parser.error(f"Build targets and seed their corpus first: {target}")
        findings = output / target / "findings"
        findings.mkdir(parents=True, exist_ok=True)
        log = output / target / "campaign.log"
        inputs = sorted(path for path in seeds.iterdir() if path.is_file())
        fingerprint = hashlib.sha256()
        for path in inputs:
            fingerprint.update(hashlib.sha256(path.read_bytes()).digest())
        command = [str(binary), str(seeds), f"-seed={options.seed}", "-max_len=65536",
                   f"-max_total_time={options.seconds}", f"-timeout={options.timeout}",
                   f"-rss_limit_mb={options.rss_mib}", "-print_final_stats=1",
                   f"-artifact_prefix={findings}/"]
        start = time.monotonic()
        timed_out = False
        with log.open("wb") as stream:
            try:
                result = subprocess.run(command, cwd=ROOT, stdin=subprocess.DEVNULL,
                                        stdout=stream, stderr=subprocess.STDOUT, env=environment,
                                        timeout=options.seconds + options.timeout + 30)
                status = result.returncode
            except subprocess.TimeoutExpired:
                timed_out, status = True, None
        text = log.read_text(errors="replace")
        counters = {key: int(value) for key, value in
                    re.findall(r"stat::([a-z_]+):\s*(\d+)", text)}
        feedback = re.findall(r"cov:\s*(\d+)\s+ft:\s*(\d+)", text)
        run = {"target": target, "command": command, "exit_code": status,
               "host_timeout": timed_out, "wall_seconds": time.monotonic() - start,
               "initial_seeds": len(inputs), "initial_corpus_sha256": fingerprint.hexdigest(),
               "binary_sha256": digest(binary), "log_sha256": digest(log),
               "statistics": counters,
               "final_coverage_feedback": int(feedback[-1][0]) if feedback else None,
               "final_feature_feedback": int(feedback[-1][1]) if feedback else None,
               "findings": sorted(str(path.relative_to(output)) for path in findings.iterdir())}
        report["runs"].append(run)
        (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(run), flush=True)
        if status != 0 or timed_out or not counters.get("number_of_executed_units"):
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
