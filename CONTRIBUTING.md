# Development

Use Rust 1.99.0 or newer with Cargo, rustfmt, and Clippy. Python 3.11+ runs the
corpus inventory and CLI conformance checks. A system Rust installation works;
rustup is optional.
The minimum Rust version matches the toolchain used for the initial validation.
Reassess it when external dependencies are introduced.

This managed workspace protects `.git` from writes. See [Git setup](docs/git-setup.md)
for the alternate local history and portable bundle. In this checkout, use
`sh tools/git-local.sh ...` in place of plain `git ...` for history operations.

```sh
cargo build --workspace --locked --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo test -p larp-compiler --release --locked --offline
python3 tools/check_corpus.py
python3 tools/run_conformance.py
python3 tools/run_conformance.py --gc-stress
python3 tools/run_conformance.py --artifacts --gc-stress
python3 tools/run_conformance.py --formatted --artifacts --gc-stress
python3 tools/run_reference.py
python3 tools/check_reproducible_build.py
git diff --check
```

`Cargo.lock` is tracked. CI installs Rust 1.99.0 explicitly. No automatic
toolchain download is needed for a compatible system compiler. New dependencies
require a concrete need, license review, and a lockfile update; offline builds
will then require a populated Cargo cache.

The dedicated reproducibility job also creates a Linux source/binary candidate
from its passed build report. `tools/package_candidate.py` checks both binaries,
normalizes archive metadata, verifies extracted checksums and executable modes,
and runs the extracted CLI/VM. The candidate retains MIT and the full tracked
source; it does not publish a versioned Release. See [distribution](docs/distribution.md).

The compiler runs scalars, closures, option/result recovery, dynamic/fixed arrays,
nominal classes, roles, extensions, isolated scenarios, and lazy bindings through syntax,
semantics, typed IR, bytecode verification, and the VM.
`tools/run_conformance.py` tests active cases through the CLI. `--gc-stress`
repeats runtime cases with collection at every object/array/closure allocation
safepoint, including lazy cells. `--artifacts` additionally builds each valid
program, loads/verifies it, and compares execution with the source path; invalid
source must fail without producing an artifact. CI includes artifact stress runs.
All 84 current fixtures are active, so `--all` selects the same cases and passes. If future cases are marked
pending, `--all` attempts them as well. Do not activate a fixture without running
it and keeping it in CI. A fully active corpus does not close the developer-preview
release gates or prove all normative rules.

Run `cargo run -p larp-cli -- run examples/callbacks.larp --gc-stress` for a
returned closure, option fallback, parsing recovery, and checked conversion.
Run `cargo run -p larp-cli -- run examples/collections.larp --gc-stress`
for an eager filter/map pipeline.
Run `cargo run -p larp-cli -- run examples/objects.larp --gc-stress` for classes,
private state, and an extracted bound method. Exercise closures, recovery,
collections, and objects with normal collection and collection at every
allocation safepoint.
The VM API exposes `Limits::gc_stress` and `execute_with_stats` for tests of
allocation, reclamation, and roots. Heap tests include generation reuse, transitive roots,
cycles across objects/arrays/closures, fixed-array payloads, and unfinished
callback outputs. Constructor initialization uses control-flow analysis before
bytecode lowering. Changes must preserve constructor escape restrictions, member
visibility, immutable `self` bindings, and effect checks through bound methods.
Role dispatch and concrete extensions preserve the same visibility and effect
contracts. Scenario changes must preserve the closed regional call graph, view
routing to performed state, immutable outer snapshots, and all-or-nothing final
publication. Run `cargo run -p larp-cli -- run examples/config-tuning.larp --gc-stress`
for a complete trial; `palantir` output uses stderr.
Run `cargo run -p larp-cli -- run examples/lazy.larp --gc-stress` for memoization
and `sybau`. Lazy initializer captures are SnapshotValues copied at declaration.
Capturing an existing lazy binding forces it immediately, including when an
ordinary closure captures it; preserve the resulting abort bound. Lazy cells
never escape into source-level APIs, fields, captures, or containers. Active
forcing frames must root their cell across nested allocations. Suppression is
dynamic across calls and silences only ordinary `yap`; it does not erase
`observe`, mute `palantir`, or hide diagnostics. Return/break lowering must unwind
only the scopes crossed, while VM return restores the caller's inherited depth. The `execute_with_observers`
API exposes separate ordinary-output and inspection sinks. Scenario VM tests
retain a private heap to check real fields after injected fatal errors.

For the reproducible scenario/explicit-copy comparison, run:

```sh
cargo run --release -p larp-compiler --example scenario_comparison --locked --offline
```

See [scenario evaluation](docs/scenario-evaluation.md) for methodology and limits.
Managed arena counters do not measure all host allocations or total memory.

`experiments/scenario-model` tests a state transition model in Rust. It does not
validate LARP region typing, scenario effects, GC, or scenario bytecode.
`tools/check_corpus.py` validates fixture metadata and paths only.

Run `cargo run -p larp-cli -- run examples/input-sum.larp -- total < scores.txt`
for arguments and line-based stdin. `io` is checked through direct/indirect calls,
roles, extensions, and callbacks; suppression does not remove it. Embedding uses
`execute_with_host` and independent optional argument/stdin capabilities. Legacy
execution APIs deny input. Preserve EOF/blank-line distinctions, record boundaries
after UTF-8 errors, failure latching, input budgets, and the runtime regional guard.
Argument array allocation must retain VM roots at GC safepoints. The conformance
schema supports declared UTF-8 stdin and argument arrays; harness runs always
provide stdin explicitly. See [host input](docs/host-input.md).

`larp fmt SOURCE --stdout` previews canonical source; `--check` checks without
writing, and no flag updates the source. The formatter requires valid syntax,
not valid types. Preserve raw token/comment sequences, parentheses, and program
order; AST annotations drive spacing but do not reorder declarations. Every
successful result is reparsed and its lexical stream compared before writing.
Tests must cover idempotence and comment boundaries, not only output snapshots.
`--formatted` in the corpus harness formats temporary copies, checks idempotence,
and executes the original contracts through the real compiler/VM. It composes
with `--artifacts` and `--gc-stress`; CI exercises the combined path. The harness
does not reformat checked-in fixtures. See [formatter policy](docs/formatter.md).

Persisted bytecode goes through `larp-artifact`, including complete source maps
and explicit wire tags. Preserve shared loader budgets and mandatory independent
verification; never hand an unchecked module to the VM. See the
[version 2 format](docs/bytecode-format-v2.md). Artifacts embed source text and
are ignored by Git. Builds publish complete sibling temporary files by rename.

Use [the roadmap](docs/roadmap.md) to choose a vertical slice. Keep the checked-in
implementation status accurate, including when tests are only design fixtures.
All repository prose and commits use English. Do not add assistant attribution.

The [practical reference](docs/reference.md) has complete runnable examples.
`tools/run_reference.py` extracts every LARP code fence and checks the ordered
contracts in `docs/reference-cases.json` through normal/stress source execution,
formatting, and loaded artifacts. Update the contract when changing a sample;
never claim execution from a syntax-only check. Evidence remains in ignored
`artifacts/reference-validation/`; use a fresh `--output` for another run.

Coverage-guided targets have their own [fuzz workspace](fuzz/README.md), pinned
tooling, and lockfile. The main workspace retains no third-party dependencies;
stable tests replay the same oracles and minimized public findings. Nightly
campaigns run on relevant main-branch pushes, weekly, or by workflow dispatch, with bounded time/RSS and saved
failure logs. Do not disable production verification or effect checks for fuzzing.
`python3 tools/seed_fuzz.py` accepts only the public example/conformance directories
and public regressions; never seed from private notes or a recursive workspace scan.

Run `python3 tools/measure_resources.py --samples 5` on Linux for measured VM
workloads, deterministic counters, fresh-process RSS, and explicit VM/host-limit
witnesses. The harness requires `cc`, builds a release probe, and records ignored
JSON/CSV evidence. See [measurement boundaries](docs/validation-resources.md):
traced allocation counts are not a memory budget, and instruction fuel does not
provide a wall-clock or host-heap bound. CI runs a one-sample-per-mode smoke check;
it does not assert timing/RSS thresholds across different runners.

LARP uses the [MIT License](LICENSE). Dependencies retain their own licenses.
The maintainer-selected remote is
[hlibstrochkovskyi/larp-lang](https://github.com/hlibstrochkovskyi/larp-lang).
