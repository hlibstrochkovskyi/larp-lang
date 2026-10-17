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
python3 tools/check_corpus.py
python3 tools/run_conformance.py
python3 tools/run_conformance.py --gc-stress
git diff --check
```

`Cargo.lock` is tracked. CI installs Rust 1.99.0 explicitly. No automatic
toolchain download is needed for a compatible system compiler. New dependencies
require a concrete need, license review, and a lockfile update; offline builds
will then require a populated Cargo cache.

The compiler runs scalars, closures, option/result recovery, and dynamic/fixed
arrays through syntax, semantics, typed IR, bytecode verification, and the VM.
`tools/run_conformance.py` tests active cases through the CLI. `--gc-stress`
repeats runtime cases with collection at every array/closure allocation
safepoint; CI runs both modes. `--all` also attempts pending features and is expected to fail until
the full specification is implemented. Do not change a fixture to active without
running it and keeping it in CI.

Run `cargo run -p larp-cli -- run examples/callbacks.larp --gc-stress` for a
returned closure, option fallback, parsing recovery, and checked conversion.
Run `cargo run -p larp-cli -- run examples/collections.larp --gc-stress`
for an eager filter/map pipeline. Compiler integration tests execute closures,
recovery, and collections with normal collection and collection at every
array/closure allocation safepoint. The VM API exposes `Limits::gc_stress` and `execute_with_stats` for tests of allocation,
reclamation, and roots. Heap tests include generation reuse, transitive roots,
array/closure cycles, fixed-array payloads, and unfinished callback outputs.
Object GC coverage remains pending.

`experiments/scenario-model` tests a state transition model in Rust. It does not
validate LARP region typing, scenario effects, GC, or scenario bytecode.
`tools/check_corpus.py` validates fixture metadata and paths only.

Use [the roadmap](docs/roadmap.md) to choose a vertical slice. Keep the checked-in
implementation status accurate, including when tests are only design fixtures.
All repository prose and commits use English. Do not add assistant attribution.

No open-source license has been chosen. This repository does not grant a license
by implication; select one before public distribution. Git has no remote until
the maintainer supplies a destination.
