# VM resource measurements

Date: 2026-10-19. Scope: bounded Linux measurements of real source compilation,
verified bytecode execution, tracing GC, and resource-failure classification.
This is a reproducible local baseline, not a performance guarantee or a release
claim.

## Reproduce

From the repository root:

```sh
python3 tools/measure_resources.py --samples 5
```

Requirements are Linux, Python 3, a C compiler named `cc`, and the workspace Rust
toolchain. No extra Python packages or Rust dependencies are required. The driver
builds the `resource_probe` example in release mode with `--locked --offline`, and
compiles a small Linux measurement launcher with warnings denied. Neither the
launcher nor the probe is part of the language implementation or CLI.

`--scale 1..10` changes workload iteration counts, `--timeout` changes each
process deadline, and `--output` chooses the report directory. `--skip-build`
reuses an existing probe binary and should only be used after building the
current sources. The default output is ignored
`artifacts/resource-measurements/`: generated LARP sources, per-run stdout,
stderr and OS usage, `measurements.json`, and `measurements.csv`. The JSON records
the Rust version, machine/CPU, binary/launcher/driver/source SHA-256 values,
commands, limits, raw counters, and individual samples. It does not contain
private design documents or inherited terminal input.

Each workload runs once per fresh process, five times in normal GC and five in
stress GC. There is no in-process warm-up. Exact output and successful final
reclamation are required. VM counters must agree across identical samples;
instruction, allocation, and reclamation counts must also agree across GC modes.
Failure witnesses require their particular error message or host failure, rather
than accepting any unsuccessful process as evidence.

## Measurement boundaries

The Rust probe reads source, compiles it through the real frontend/typed IR and
mandatory verifier, and invokes `execute_with_stats`. Its `vm_elapsed_ns` surrounds
VM execution, including final successful collection, and excludes compilation.
`process_wall_ns` includes process/launcher startup, source loading, compilation,
verification, execution, and result serialization. The Python polling interval
adds up to approximately one millisecond of observation latency to that wall
measurement. Neither value is a CPU-cycle count.

Peak process RSS comes from Linux `wait4` on the actual probe process, in KiB
converted to bytes. A native launcher executes first and then forks the measured
child. This matters: directly forking a child from Python can retain the Python
parent's pre-exec RSS high-water mark, obscuring a small VM's memory use. The
launcher reports its child's usage; the Python parent's separate wrapper usage
is preserved under `launcher_peak_rss_bytes` and is not labeled as VM RSS.
RSS covers compilation and VM execution together and is not a VM-only peak.

Each normal workload gets a 256 MiB `RLIMIT_AS` address-space cap, a 20-second
wall deadline, 100 million VM instructions, and 1,024 VM frames. A process group
allows the driver to kill both launcher and child on timeout. Core files are
disabled. These OS controls belong to the measurement harness: embedders and the
ordinary CLI do not acquire these controls automatically.

VM `allocations`/`peak_live_objects` count traced heap entries, not bytes or every
host allocation. In particular text buffers, fixed-array value copies, VM
stacks, compiler structures, and callback output buffers consume host memory
without necessarily increasing those counters. Successful final reclamation
establishes that the VM reports no retained traced entries, not that process RSS
returns to its initial value.

## Executed baseline

Executed 100 fresh child processes: seven successful workloads, two VM-limit
witnesses, and one host-memory witness, each in two GC modes with five samples.
Machine: AMD Ryzen 5 PRO 4650U, x86-64 Linux 7.2.9, glibc 2.44. Toolchain:
Rust 1.99.0, LLVM 23.1.1. Probe SHA-256:
`550711e2ad191a7c91c6753dbcef0a2e52d1ab2a507ceec73f8d916521e23c8c`.

All successful workloads executed their assertions and produced exact expected
stdout in both modes. All counters were deterministic under the checks above.
Timings below are medians of five VM measurements; RSS is the largest observed
whole-process peak in each group. This was a shared, unpinned machine without
frequency or background-load control. Differences, including apparently faster
stress runs, must not be treated as reliable comparisons.

| Workload at scale 1 | VM ms normal / stress | Peak RSS MiB normal / stress |
| --- | ---: | ---: |
| 100,000 checked scalar increments | 44.846 / 35.896 | 3.035 / 3.070 |
| 10,000 temporary dynamic arrays | 8.575 / 13.579 | 3.137 / 3.070 |
| 1,500 retained linked objects | 3.379 / 24.040 | 3.320 / 3.324 |
| 2,000 inputs through filter/map/forEach | 6.587 / 7.114 | 3.258 / 3.258 |
| 2,000 copies of 32 fixed text elements | 18.600 / 17.556 | 3.168 / 3.172 |
| 5,000 lazy cells, each forced twice | 6.272 / 15.450 | 3.074 / 3.066 |
| 3,000 isolated scenarios and selective commits | 7.684 / 2.956 | 3.133 / 3.137 |

| Workload | Instructions | Traced allocations = reclaimed | Collections normal / stress | Peak traced entries normal / stress |
| --- | ---: | ---: | ---: | ---: |
| Scalar increments | 3,100,027 | 0 | 1 / 1 | 0 / 0 |
| Temporary arrays | 470,027 | 10,000 | 159 / 10,001 | 64 / 2 |
| Retained chain | 55,530 | 1,500 | 6 / 1,501 | 1,500 / 1,500 |
| Collection callbacks | 125,063 | 7 | 1 / 9 | 7 / 7 |
| Fixed text copies | 126,148 | 0 | 1 / 2 | 0 / 0 |
| Lazy memoization | 320,027 | 5,000 | 80 / 5,001 | 64 / 2 |
| Scenarios | 177,056 | 1 | 1 / 2 | 1 / 1 |

The retained chain remains reachable until successful VM termination. Stress GC
therefore repeatedly scans a growing live graph; it cannot collect live entries.
The temporary-array and lazy workloads instead demonstrate reclamation of
unreachable entries during execution. Fixed text copies demonstrate meaningful
host memory work despite zero traced allocations. Scenario repetition reuses
one real object and checks that only the selected integer field survives; its
performed text remains discarded.

## Limit witnesses and remaining risks

All ten infinite-loop runs terminated with `LARP-R006`, specifically
`Instruction budget exhausted`, under a budget of 100 instructions. All ten
recursive runs terminated with `LARP-R006`, specifically `Call-frame limit
exceeded`, under a limit of 64 frames. These are structured VM diagnostics; they
are not host crashes. The VM currently returns successful-run counters only, so
the probe reports `stats: null` for these failures rather than inventing partial
statistics.

All ten text-doubling runs hit the separate 64 MiB host address-space cap and
terminated with `SIGABRT`. Rust reported a failed 16 MiB allocation; maximum RSS
was 59.488 MiB. The harness identifies this as `host_memory_failure`, never as
`LARP-R006` or a recoverable language result. No core file was written. RSS and
virtual address space are different quantities, so a 64 MiB address-space cap
does not imply a 64 MiB resident peak. GC stress did not prevent this failure:
the growing text is owned value storage outside the traced object count.

These witnesses establish limits of the current controls. Instruction fuel is
not a heap-byte limit or a wall-clock deadline, and a single instruction can
perform substantial copying, sorting, tracing, text scanning, or blocking host
I/O. Call-frame limits do not bound every value's size. Host allocation failure
can abort the process. There is currently no VM-wide byte budget, interruptible
intrinsic protocol, OOM recovery guarantee, or in-process untrusted-code sandbox.
The bounded harness avoids exposing the machine to unbounded growth; it does not
add those guarantees to the runtime.

The harness timeout path was separately exercised with
`--samples 1 --timeout 0.001 --skip-build --output artifacts/resource-timeout`.
It killed the process group, recorded `timed_out: true` and `SIGKILL`, preserved
the partial report, and rejected the run as a host wall-clock timeout. No probe
process remained. A missing child RSS after forced group termination stays
`null`, rather than being substituted with the wrapper's inherited high-water.

This baseline does not cover maximum-size artifacts, arbitrary nested value-copy
amplification, every recursive callback graph, asynchronous interruption,
long-running fragmentation, Windows/macOS behavior, or blocking input duration.
Those require further measurements and, where appropriate, runtime changes.
