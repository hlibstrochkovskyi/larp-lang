# Coverage-guided robustness checks

[Project overview](../README.md) · [Contributing](../CONTRIBUTING.md) · [Saved regressions](regressions/README.md)

This standalone Cargo workspace keeps libFuzzer's third-party tooling out of
the dependency-free compiler workspace. The shared oracles live in
`crates/larp-fuzz-support`; ordinary stable workspace tests replay deterministic
mutations and checked-in regressions without downloading fuzz dependencies.

## Stable regression checks

From the repository root, run the shared oracles and saved regression replay:

```sh
cargo test -p larp-fuzz-support --locked --offline
```

This is a deterministic test suite, not a coverage-guided campaign. Ordinary
contributors do not need nightly Rust or `cargo-fuzz` for it.

## Instrumented campaigns

Install a C++ compiler, Rust `nightly-2026-10-17`, and `cargo-fuzz` 0.13.2.
Run these commands from the repository root, using a fresh corpus directory:

```sh
rustup toolchain install nightly-2026-10-17 --profile minimal
cargo install cargo-fuzz --version 0.13.2 --locked
cargo build --locked --offline
python3 tools/seed_fuzz.py
cargo +nightly-2026-10-17 fetch --manifest-path fuzz/Cargo.toml --locked
cargo +nightly-2026-10-17 fuzz build --codegen-units 16
cargo +nightly-2026-10-17 fuzz run frontend --codegen-units 16 -- \
    -seed=49 -max_len=65536 -max_total_time=60 -timeout=5 \
    -rss_limit_mb=1024 -print_final_stats=1
```

Repeat the last command for `formatter`, `compiler`, and `artifact`. Use
`CARGO_NET_OFFLINE=true` after fetching the standalone lock's dependencies.
The normal command uses AddressSanitizer, coverage instrumentation, debug
assertions, and overflow checks. Do not use `--release` to disable assertions.
No production checks are conditional on `cfg(fuzzing)`.

After building, `python3 tools/run_fuzz.py --seconds 60 --seed 49` runs all four
instrumented binaries sequentially and records commands, binary/corpus hashes,
feedback counters, failure artifacts, and logs under `artifacts/fuzz-campaign/`.
It rejects nonzero exits, missing execution counters, and host timeouts. The
default executable directory is the Linux x86-64 cargo-fuzz target directory;
override `--binary-dir` for another host. The scoped push/weekly/manual GitHub workflow saves
this evidence. Stable CI also runs regression replay without nightly tooling.
Report directories must be empty; select a fresh `--output` for every campaign.
Seeding also requires an empty output, avoiding silent reuse of a mutated corpus.
To continue a search, pass the old corpus to the runner explicitly and retain its
recorded initial fingerprint. Such a run starts from a warmed corpus rather than
the original public seed inventory.

The [bounded fuzz workflow](../.github/workflows/fuzz.yml) pins the CI toolchain,
target list, and campaign settings. A successful run is evidence for that
particular build and input stream, rather than a permanent project-wide result.

On hosts that prohibit LeakSanitizer's required ptrace support, explicitly use
`--disable-leak-check` (or `ASAN_OPTIONS=detect_leaks=0` for `cargo fuzz run`). This
leaves AddressSanitizer's memory-access checks enabled but supplies no leak-check
evidence; the runner records the effective sanitizer environment. Do not count a
fatal LeakSanitizer startup/shutdown error as a successful campaign or a language
defect. The default leaves leak checking enabled.

## Oracles and input scope

| Target | Oracle |
| --- | --- |
| `frontend` | Lexer/parser termination, valid diagnostic ranges, ordered non-overlapping token/comment spans and EOF |
| `formatter` | Valid syntax, exact token/comment spellings and order, idempotence; internal preservation failures abort |
| `compiler` | Normal source errors are allowed; compiler-generated invalid bytecode aborts; successful artifacts round-trip canonically |
| `artifact` | Raw binary decoding and outer-length-adjusted decoding; every accepted module passes the production verifier and deterministic re-encoding |

All targets admit at most 64 KiB before allocating a source. Non-UTF-8 source
candidates are rejected; binary artifacts may contain any byte. The loader's
additional candidate changes only the external payload length, so insertions and
deletions can reach section parsing. It never bypasses type/effect verification.
Artifact resource-limit errors are expected, including valid compiler output
that exceeds persistence limits. Other encoding errors are invariant failures.

Seeds come only from `examples/`, `tests/conformance/`, and checked-in public
regressions. The seed tool compiles artifacts; it never executes those programs.
Active fixture compile-error contracts must hold; all other active fixtures and
public examples must compile. Directory markers are not regression inputs.
Mutable corpus, crashes, build products, and coverage outputs are ignored.
Do not publish an automatically generated crash before checking its contents.

## Recording findings

For a finding, first replay the exact input with `cargo fuzz run TARGET FILE`,
then minimize it with `cargo fuzz tmin TARGET FILE`. Fix the production defect
and add a small input under `fuzz/regressions/TARGET/`, with the cause and expected
behavior recorded in `regressions/README.md`. Workspace tests replay every file
in those four directories, so a discovered defect remains covered on stable CI.

## Limits and tooling licenses

These are bounded bug searches, not proof of language conformance, memory safety,
or sandboxing. Fuel does not bound allocation, so the targets do not execute
arbitrary VM programs. libFuzzer's RSS/time controls bound campaigns, not host
capabilities or all resources of individual compiler operations. Coverage counters
are instrumentation feedback, not line/branch coverage percentages. A fixed seed
helps reproduce an input stream but does not guarantee identical time-limited runs
across machines or toolchains.

Tooling references: [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/guide.html)
and [LLVM libFuzzer options](https://llvm.org/docs/LibFuzzer.html#options).
The pinned `libfuzzer-sys` 0.4.13 declares `(MIT OR Apache-2.0) AND NCSA`;
`cargo-fuzz` declares `MIT OR Apache-2.0`. These dependencies retain their own
licenses; LARP's original code uses the [MIT License](../LICENSE).
