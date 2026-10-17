# Foundation validation

Historical bootstrap snapshot. See [implementation status](implementation-status.md)
and [scalar-core validation](validation-scalar-core.md) for the current compiler.

Date: 2026-10-17. Environment: Linux, system rustc 1.99.0, Cargo 1.99.0,
rustfmt 1.10.0, Clippy 0.1.99. No third-party Rust dependencies.

## Local results

| Check | Result |
| --- | --- |
| `cargo build --workspace --locked --offline` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | Passed: 15 tests, no ignored tests |
| `python3 tools/check_corpus.py` | Passed: 28 pending fixture definitions |
| `cargo run -p larp-cli --locked --offline -- explain LARP-T041` | Printed the code, explanation, and checked-conversion guidance |

Test distribution: 3 source-location tests, 1 diagnostic-registry test,
3 CLI integration tests, and 8 scenario-model tests. The model includes the full
selection matrix for three fields, discard/unwind, last claims, sequential
baselines, conditional configuration acceptance, and an invariant counterexample.

The private brief is ignored by Git. Author and committer identity resolve to the
maintainer's preexisting configuration. No remote was configured, no push was
performed, and hosted CI has not run. A workflow is checked in for future runs.

The managed filesystem allowed initialization but rejected `.git/index.lock`
creation during staging. History is saved separately in `.git-foundation/`, with
a portable bundle in `artifacts/`. See [Git setup](git-setup.md). Plain `git`
still sees the protected, unborn standard repository.

## Limits of the evidence

These checks establish a buildable engineering foundation and a tested scalar
state-transition experiment. They do not establish language conformance, parser
correctness, effect/region soundness, bytecode verification, GC correctness, or
performance. All 28 `.larp` cases remain pending; inventory validation is not
compilation. Specification v0.1 and the compiler architecture are proposals.
