# Scenario evaluation

Status: both examples and their explicit-copy alternatives compile through typed
IR and verified bytecode and execute on the LARP VM. The measurements below were
recorded on 2026-10-18. These small examples establish behavior and implementation
cost; they do not establish general demand for a scenario feature.

## Configuration tuning without external I/O

Run [config-tuning.larp](../examples/config-tuning.larp) and
[config-tuning-explicit.larp](../examples/config-tuning-explicit.larp):

```sh
cargo run -p larp-cli -- run examples/config-tuning.larp --gc-stress
cargo run -p larp-cli -- run examples/config-tuning-explicit.larp --gc-stress
```

Both initialize `Plan(workers=2, retryDelayMs=1000)` and a budget of 100. The trial
uses workers=8 and retryDelayMs=250, then computes `workers * 12`. Only workers
persist when the estimated cost fits the budget. Both executions print:

```text
8
1000
```

The scenario additionally emits this report to stderr before commit:

```text
PALANTIR [CapacityTrial]
PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED
workers | 2 | 8 | 8 | true
retryDelayMs | 1000 | 250 | - | false
```

The explicit version constructs one trial object by reading both current fields,
mutates that copy, then manually copies the accepted workers value back. The
scenario version names the original root once, performs its changes through the
isolated view, and selects workers with `canon`. `flex` records the claimed
workers value; the ordinary delay write does not create a claim. The estimator
operates on integers; neither example benchmarks a service or contacts a host.

A separately executed budget=90 variant prints `2\n1000\n`: the workers claim
remains visible, but neither field is selected. Calculating the budget before
entry is uncomplicated here. A workload that needs live outside configuration,
network measurements, or a saved scenario-local result would encounter stricter
limits and may be better expressed with ordinary objects and functions.

## Game-stat trial

Run [game-training.larp](../examples/game-training.larp) and
[game-training-explicit.larp](../examples/game-training-explicit.larp). Both start
with `Stats(power=10, visualBonus=0)`, add 5 power, claim a visual bonus of 500, and
accept only power when `power * 2 >= 30`. Both print:

```text
15
0
```

The scenario report is:

```text
PALANTIR [TrainingTrial]
PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED
power | 10 | 15 | - | true
visualBonus | 0 | 500 | 500 | false
```

This case distinguishes an asserted appearance from accepted state. The explicit
version has the same final values, but the trial object's fields do not retain
that distinction unless the program adds separate claim storage. Neither model
includes inventory, conservation laws, or other cross-field invariants.

## Source complexity and failure behavior

The counts below include nonempty, noncomment physical lines, including braces.
The trial markers delimit the measured portion; constructors and initial root
allocation are outside it.

| Program | Complete source | Trial | Trial without inspection |
| --- | ---: | ---: | ---: |
| Configuration scenario | 23 | 9 | 8 |
| Configuration explicit copy | 21 | 7 | 7 |
| Game scenario | 22 | 9 | 8 |
| Game explicit copy | 20 | 7 | 7 |

The scenarios are slightly longer. Their benefit is a checked mutation boundary,
explicit selection, and common inspection, rather than fewer source lines. A
reader of the explicit versions must track a constructor copy and a later field
copy; a reader of the scenario versions must understand delayed selection and
normal-fallthrough commit. These are readability judgments about the examples,
not results from a user study.

A manual copy isolates the trial object's ordinary writes. It does not prevent an
early write to `current`, an accidental live alias read, or a callback mutating a
referenced object. The author must delay every real write until all potentially
failing work is complete. If several accepted fields must publish together, the
author must prepare their final values before performing those writes. A
constructor copy also needs updating when fields are added to the class.

The scenario imposes its boundary in the compiler and verifier. An executed
variation reading `alias.workers` from an outer alias produced `LARP-S002`; one
assigning the imported budget produced `LARP-S004`, both at the offending source
span. The compiler tests also cover allocation hidden behind calls, unsafe
indirect targets, role dispatch, outer stores, and exits crossing the boundary.
See [scenario compiler tests](../crates/larp-compiler/tests/scenarios.rs) and
[bytecode scenario tests](../crates/larp-bytecode/tests/scenarios.rs).

Injecting `crashout("trial aborted")` after the configuration report produced
`LARP-R003`, retained the report, and skipped final stdout. That CLI observation
alone cannot prove the original object's fields survived: the process terminates.
The [VM tests](../crates/larp-vm/src/scenario_tests.rs) retain the private runtime
heap and inspect the actual original fields after abort, assertion failure,
overflow, instruction exhaustion, and inspection-sink failure. They remain at
the baseline even after selection. A normal `fumble(...)` value does not cancel a
scenario; only fatal execution prevents its fallthrough commit.

Selective commit can still violate application invariants. Starting with
balance=100 and reserved=20, performing balance=10 and reserved=0, then selecting
only balance leaves balance=10 and reserved=20. Both fields retain valid types.
The compiler regression `selective_commit_preserves_field_types_but_can_break_cross_field_invariants`
executes this counterexample. There is no group validation or whole-object
invariant guarantee in the current feature.

## Reproducible VM comparison

The runnable [benchmark example](../crates/larp-compiler/examples/scenario_comparison.rs)
loads these four actual `.larp` files. It changes only the body between
`// Begin trial.` and `// End trial.`, wrapping it in
`npc loop (mf comparisonIteration in 0..10000)`. The root is initialized once
outside the loop and retains its identity throughout each VM run.

```sh
cargo run --release -p larp-compiler --example scenario_comparison --locked --offline > /tmp/scenario-comparison.csv
```

Each variant compiles and verifies once, executes one warmup, then records seven
sequential wall-clock samples with `Instant`. Compilation and source rewriting
are excluded from elapsed time. Each timed VM run includes root construction,
the range loop, 10,000 trials, final stdout formatting, and final collection.
Normal GC thresholds are used, with a 100-million-instruction execution budget;
this timing run does not use `--gc-stress`.

The primary comparison removes each `palantir` statement from its trial body,
so neither alternative formats inspection output. Separate scenario variants
retain it and direct inspection to `std::io::sink()`: formatting still executes,
but no terminal or file output is measured. Explicit-copy variants have no
inspection report. The benchmark checks the paired stdout after every run,
requires all managed objects to be reclaimed, and checks deterministic VM
statistics across warmup and samples.

Configuration remains `8\n1000\n` after repetition. Game training accumulates on
the retained root and ends at `50010\n0\n`, rather than resetting to 10 each
iteration. These are repeated updates, not 10,000 independent fresh-root trials.
The inserted range loop and any compiler-generated value imports are part of
the measured program in both alternatives.

The recorded machine was Linux 7.2.9-arch1-1, x86_64, AMD Ryzen 5 PRO 4650U
(6 cores, 12 logical CPUs). Rust was 1.99.0 (`b940084d7`, Arch Linux
`1:1.99.0-1`), Cargo 1.99.0. The Rust toolchain used its release profile; the LARP
programs still execute bytecode, with no native backend or optimizer. CPU
frequency, process affinity, and other host activity were not controlled. Variants
ran in a fixed order rather than randomized or interleaved rounds.
These measurements describe one local run and make no statistical significance
or cross-machine performance claim.

Raw samples are in [scenario-comparison.csv](data/scenario-comparison.csv).
Durations below are milliseconds for the complete 10,000-trial VM run:

| Program | Inspection | Median ms | Min–max ms | VM instructions | Managed allocations | Peak managed objects |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Configuration scenario | Disabled | 12.381 | 12.012–13.948 | 800,060 | 1 | 1 |
| Configuration explicit copy | None | 14.644 | 14.329–16.362 | 1,000,060 | 10,001 | 64 |
| Game scenario | Disabled | 11.767 | 11.435–12.736 | 840,056 | 1 | 1 |
| Game explicit copy | None | 16.365 | 15.980–16.455 | 1,070,056 | 10,001 | 64 |
| Configuration scenario | Enabled, discarded | 13.484 | 12.837–15.310 | 850,060 | 1 | 1 |
| Game scenario | Enabled, discarded | 12.547 | 12.469–13.582 | 890,056 | 1 | 1 |

In these samples, the scenario paths have lower elapsed medians and execute fewer
VM instructions. They avoid creating a traced trial object on every iteration;
the explicit-copy paths allocate 10,000 such objects plus the original root.
Normal collection reaches 64 live arena objects in those paths. All variants
reclaim every managed allocation by exit. This does not establish that scenarios
are inherently faster than explicit state management: scalar local snapshots,
a value-oriented record type, or future allocation elimination could materially
change the baseline.

Managed allocation and peak counts describe entries in the traced object/array/
closure arena, not bytes or whole-process memory. Scenario B/P/claim/selection
vectors, prepared commit buffers, VM frames, locals, text backing storage, and
other Rust allocations are outside these counters. Those allocations still cost
time and memory. RSS, allocator bytes, and snapshot storage were not measured;
the table cannot support a claim that scenarios use less total memory.

## Decision

Keep the current value-only scenario feature experimental. These executions
show that checked lexical isolation, final-field selection, claim retention, and
inspection can work together on the real compiler/VM, and the measured toy
workloads do not expose an excessive elapsed-time cost. The comparison does not
justify arbitrary object-graph snapshots, nested transactions, or stronger safety
claims. The small explicit-copy programs remain viable alternatives, and the
restrictions can make outside-state-heavy workflows awkward.

Before expanding eligibility, find a substantial application whose trial/commit
boundary is clearer with this feature, measure host allocations as well as arena
counts, and evaluate grouped invariants. The rest of the language and toolchain
must remain useful if that evaluation favors explicit state management.
