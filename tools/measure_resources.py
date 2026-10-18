#!/usr/bin/env python3
"""Measure bounded real LARP workloads in independent Linux subprocesses."""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class Workload:
    name: str
    source: str
    output: str = ""
    expected: str = "ok"
    expected_message: str | None = None
    fuel: int = 100_000_000
    frames: int = 1024
    memory_mib: int = 256


def workloads(scale: int) -> list[Workload]:
    """Keep inputs explicit; generated files remain in the ignored report tree."""
    cpu, churn, retained = 100_000 * scale, 10_000 * scale, 1500 * scale
    callbacks, copies, lazy, trials = 2000 * scale, 2000 * scale, 5000 * scale, 3000 * scale
    fixed = ", ".join('"' + "x" * 64 + '"' for _ in range(32))
    return [
        Workload("cpu", f"""main_character {{
    mf total = 0;
    npc loop (mf i in 0..{cpu}) {{ total += 1; }}
    yap(total);
}}
""", f"{cpu}\n"),
        Workload("heap-churn", f"""main_character {{
    mf total = 0;
    npc loop (mf i in 0..{churn}) {{
        mfs<mf> temporary = [i, 1, 2];
        total += temporary.size();
    }}
    yap(total);
}}
""", f"{churn * 3}\n"),
        Workload("retained-chain", f"""archetype Link {{
    highkey baddie<Link> previous;
    init(baddie<Link> previous) {{ self.previous = previous; }}
}}
main_character {{
    baddie<Link> head = no_bitches;
    npc loop (mf i in 0..{retained}) {{ head = some(new Link(head)); }}
    yap(head.isSome());
}}
""", "fr\n"),
        Workload("callbacks", f"""main_character {{
    mfs<mf> input = [];
    npc loop (mf i in 0..{callbacks}) {{ input.push(i); }}
    mfs<mf> total = [0];
    input.filter(x => x % 2 == 0).map(x => x + 1)
        .forEach(x => {{ total[0] += x; }});
    yap(total[0]);
}}
""", f"{(callbacks // 2) ** 2}\n"),
        Workload("fixed-value-copies", f"""main_character {{
    hediJeans<text, 32> original = [{fixed}];
    mf total = 0;
    npc loop (mf i in 0..{copies}) {{
        hediJeans<text, 32> copied = original;
        copied[0] = "changed";
        total += copied[0].size();
    }}
    no_cap(original[0].size() == 64);
    yap(total);
}}
""", f"{copies * 7}\n"),
        Workload("lazy-memoization", f"""main_character {{
    unc total = 0;
    npc loop (mf i in 0..{lazy}) {{
        nonchalant mf value = i + 1;
        total += fraud<unc>(value);
        total += fraud<unc>(value);
    }}
    yap(total);
}}
""", f"{lazy * (lazy + 1)}\n"),
        Workload("scenario", f"""archetype Counter {{
    highkey mf value;
    highkey text label;
    init() {{ self.value = 0; self.label = "real"; }}
}}
main_character {{
    Counter counter = new Counter();
    npc loop (mf i in 0..{trials}) {{
        larp (counter) as Measurement {{
            flex counter.label = "performed";
            counter.value += 1;
            canon counter.value;
        }}
    }}
    no_cap(counter.label == "real");
    yap(counter.value);
}}
""", f"{trials}\n"),
        Workload("fuel-limit", "main_character { goon {} }\n", expected="LARP-R006",
                 expected_message="Instruction budget exhausted", fuel=100),
        Workload("frame-limit", """sidequest recurse() -> mf { return recurse(); }
main_character { yap(recurse()); }
""", expected="LARP-R006", expected_message="Call-frame limit exceeded", frames=64),
        Workload("host-memory-limit", """main_character {
    text growing = "x";
    npc loop (mf i in 0..28) { growing = growing + growing; }
    yap(growing.size());
}
""", expected="host_memory_failure", memory_mib=64),
    ]


def run_probe(command: list[str], directory: Path, timeout: float, memory_mib: int,
              launcher: Path) -> dict:
    """The exec'd launcher forks a new child before collecting its wait4 usage."""
    stdout_path, stderr_path = directory / "stdout.json", directory / "stderr.txt"
    metrics_path = directory / "process.json"
    metrics_path.unlink(missing_ok=True)
    start = time.monotonic_ns()
    timed_out = False
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        process = subprocess.Popen(
            [str(launcher), str(metrics_path), str(memory_mib), *command],
            stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
            start_new_session=True,
        )
        while True:
            pid, status, usage = os.wait4(process.pid, os.WNOHANG)
            if pid:
                break
            if time.monotonic_ns() - start > timeout * 1_000_000_000:
                timed_out = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                _, status, usage = os.wait4(process.pid, 0)
                break
            time.sleep(0.001)
        process.returncode = os.waitstatus_to_exitcode(status)
    elapsed = time.monotonic_ns() - start
    stdout_text, stderr_text = stdout_path.read_text(), stderr_path.read_text(errors="replace")
    child = json.loads(metrics_path.read_text()) if metrics_path.exists() else None
    result = {
        "command": command,
        "process_exit_code": child["process_exit_code"] if child else process.returncode,
        "launcher_exit_code": process.returncode,
        "timed_out": timed_out, "process_wall_ns": elapsed,
        "process_cpu_user_seconds": child["user_seconds"] if child else None,
        "process_cpu_system_seconds": child["system_seconds"] if child else None,
        "process_peak_rss_bytes": child["peak_rss_kib"] * 1024 if child else None,
        "launcher_peak_rss_bytes": usage.ru_maxrss * 1024,
        "address_space_limit_mib": memory_mib, "probe": None,
        "stderr": stderr_text,
    }
    if result["process_exit_code"] == 0 and not timed_out:
        result["probe"] = json.loads(stdout_text)
    return result


def validate(result: dict, workload: Workload) -> None:
    if result["timed_out"]:
        raise RuntimeError(f"{workload.name}: host wall-clock timeout")
    if workload.expected == "host_memory_failure":
        if result["process_exit_code"] != -signal.SIGABRT or "memory allocation" not in result["stderr"]:
            raise RuntimeError(f"{workload.name}: expected a Rust allocation abort under RLIMIT_AS: {result}")
        result["outcome"] = "host_memory_failure"
        return
    probe = result["probe"]
    if result["process_exit_code"] != 0 or probe is None:
        raise RuntimeError(f"{workload.name}: host failure: {result}")
    if probe["output"] != workload.output:
        raise RuntimeError(f"{workload.name}: unexpected stdout: {probe['output']!r}")
    if workload.expected == "ok":
        if probe["status"] != "ok" or probe["stats"]["allocations"] != probe["stats"]["reclaimed"]:
            raise RuntimeError(f"{workload.name}: failed execution or retained managed objects: {probe}")
    elif probe["status"] != "runtime_error" or probe["code"] != workload.expected:
        raise RuntimeError(f"{workload.name}: wrong runtime failure: {probe}")
    if workload.expected_message is not None and probe.get("message") != workload.expected_message:
        raise RuntimeError(f"{workload.name}: wrong runtime resource boundary: {probe}")
    result["outcome"] = workload.expected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--scale", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=20)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/resource-measurements")
    parser.add_argument("--skip-build", action="store_true")
    options = parser.parse_args()
    if sys.platform != "linux" or not hasattr(os, "wait4"):
        parser.error("This measurement protocol requires Linux wait4 and RLIMIT_AS")
    if not 1 <= options.samples <= 20 or not 1 <= options.scale <= 10 or options.timeout <= 0:
        parser.error("Require 1..20 samples, 1..10 scale, and a positive timeout")
    options.output = options.output.resolve()
    options.output.mkdir(parents=True, exist_ok=True)
    launcher = options.output / "resource-launcher"
    subprocess.run([
        "cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
        str(ROOT / "tools/resource_launcher.c"), "-o", str(launcher),
    ], check=True)
    if not options.skip_build:
        subprocess.run([
            "cargo", "build", "-p", "larp-compiler", "--example", "resource_probe",
            "--release", "--locked", "--offline",
        ], cwd=ROOT, check=True)
    binary = ROOT / "target/release/examples/resource_probe"
    metadata = {
        "schema_version": 1, "created_unix_seconds": time.time(),
        "platform": platform.platform(), "machine": platform.machine(),
        "python": sys.version, "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "launcher_sha256": hashlib.sha256(launcher.read_bytes()).hexdigest(),
        "driver_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "cpu": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name")), "unknown"),
        "samples": options.samples, "scale": options.scale, "timeout_seconds": options.timeout,
        "process_measurement": "Exec'd native launcher forks a fresh child; Linux wait4 ru_maxrss KiB converted to bytes; includes source read/compile/verify/VM",
        "vm_measurement": "Instant around execute_with_stats, including final successful collection; excludes source compile",
        "runs": [],
    }
    rows = []
    deterministic = {}
    for workload in workloads(options.scale):
        source = options.output / f"{workload.name}.larp"
        source.write_text(workload.source, encoding="utf-8")
        for mode in ("normal", "stress"):
            for sample in range(1, options.samples + 1):
                directory = options.output / workload.name / mode / str(sample)
                directory.mkdir(parents=True, exist_ok=True)
                result = run_probe([
                    str(binary), str(source), str(workload.fuel), str(workload.frames), mode,
                ], directory, options.timeout, workload.memory_mib, launcher)
                result.update({"workload": workload.name, "mode": mode, "sample": sample,
                               "source_sha256": hashlib.sha256(workload.source.encode()).hexdigest()})
                metadata["runs"].append(result)
                # Persist evidence even if validation of this run fails.
                (options.output / "measurements.json").write_text(json.dumps(metadata, indent=2) + "\n")
                validate(result, workload)
                probe = result["probe"] or {}
                if probe.get("stats") is not None:
                    stats = probe["stats"]
                    key = (workload.name, mode)
                    if key in deterministic and deterministic[key] != stats:
                        raise RuntimeError(f"{workload.name}: counters changed across identical runs")
                    deterministic[key] = stats
                    other = deterministic.get((workload.name, "normal"))
                    if other is not None and any(stats[name] != other[name] for name in (
                            "instructions", "allocations", "reclaimed")):
                        raise RuntimeError(f"{workload.name}: GC mode changed execution/allocation counts")
                rows.append({
                    "workload": workload.name, "mode": mode, "sample": sample,
                    "outcome": result["outcome"], "process_wall_ns": result["process_wall_ns"],
                    "process_peak_rss_bytes": result["process_peak_rss_bytes"],
                    "vm_elapsed_ns": probe.get("vm_elapsed_ns", ""),
                    **{name: (probe.get("stats") or {}).get(name, "") for name in (
                        "instructions", "allocations", "collections", "reclaimed", "peak_live_objects")},
                })
            group = [row for row in rows if row["workload"] == workload.name and row["mode"] == mode]
            print(f"{workload.name:20s} {mode:6s}: {workload.expected}, "
                  f"median process {statistics.median(row['process_wall_ns'] for row in group) / 1e6:.2f} ms, "
                  f"peak RSS {max(row['process_peak_rss_bytes'] for row in group) / 1048576:.2f} MiB", flush=True)
    (options.output / "measurements.json").write_text(json.dumps(metadata, indent=2) + "\n")
    with (options.output / "measurements.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
