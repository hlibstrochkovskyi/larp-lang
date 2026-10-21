# Contributing to LARP

LARP has a real compiler and VM, and plenty left to build. Small bug fixes,
clearer diagnostics, executable examples, documentation, and language-design
feedback are all welcome. You do not need to understand the whole compiler to
make a useful contribution.

The first public prerelease is
[v0.1.0-preview.1](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.1.0-preview.1).
Current development uses `0.2.0-preview.1` and adds [file modules](docs/modules.md).
The language and artifact formats remain experimental. Read the
[implementation status](docs/implementation-status.md) for what works today;
the [proposed specification](docs/spec/v0.1.md) also describes rules that need
continued review.

## Get a working checkout

Install Rust **1.99.0 or newer**, Cargo, rustfmt, Clippy, and Python **3.11 or
newer**. CI uses Rust 1.99.0. A compatible system installation works; rustup is
optional. The main Cargo workspace has no third-party dependencies, so its
locked builds work offline. The separate fuzz workspace has additional setup.

```sh
git clone https://github.com/hlibstrochkovskyi/larp-lang.git
cd larp-lang
cargo build --workspace --locked --offline
target/debug/larp run examples/hello.larp
target/debug/larp run examples/config-tuning.larp --gc-stress
```

`config-tuning.larp` demonstrates alternative state and selective commit;
`palantir` writes its inspection report to stderr. Browse the
[examples](examples/) and [practical reference](docs/reference.md), then use the
[architecture and crate map](docs/architecture.md#workspace-boundaries) to find
the relevant implementation.

Normal clones use ordinary Git commands. Only the original managed development
checkout uses the alternate metadata described in [Git setup](docs/git-setup.md).
That arrangement is not required to build or contribute.

## Choose a change

For a bug, a small reproducer is the best starting point. For a language feature,
open an [issue](https://github.com/hlibstrochkovskyi/larp-lang/issues) describing
the program you want to write and its intended behavior before building a large
implementation. The [roadmap](docs/roadmap.md) gives the current sequence of work.

When reporting a problem, include:

- The smallest complete `.larp` program or exact CLI command that reproduces it.
- Expected behavior and actual stdout, stderr, and exit status.
- `larp --version --json`, or the source commit for a development build.
- Your OS, architecture, and `rustc -Vv` if the problem concerns building.
- Whether it happens with source, a `.larpbc` artifact, or `--gc-stress`.

Include diagnostic codes and source locations as text. Artifacts embed their
original source, so check their contents before attaching them. Keep unrelated
files, credentials, and private notes out of reports.

## Prepare a focused pull request

1. Create a branch from current `main` and keep the change about one problem.
2. For compiler/runtime work, read the [specification](docs/spec/v0.1.md),
   [architecture](docs/architecture.md), and [roadmap](docs/roadmap.md). Preserve
   the relevant [engineering contracts](docs/development.md#contracts-to-preserve).
3. Add an appropriate regression or executable acceptance case. Keep diagnostics,
   documentation, and implementation status consistent with the actual behavior.
4. Run the checks below and describe what you verified in the PR.

Write repository prose, code comments, diagnostics, test descriptions, and commit
messages in English. Use your own Git identity. Keep commits focused and do not include
build output, generated evidence, or private design notes.

A useful PR description states the concrete problem, the new behavior, and the
checks run. Explain any compatibility change or remaining limitation. If you
change an observable language rule, include a before/after example and its
acceptance criteria. New dependencies need a concrete purpose, license review,
and a tracked lockfile update.

## Check your change

For compiler or foundation changes, run this required local baseline from the
repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
python3 tools/check_corpus.py
python3 tools/run_conformance.py
git diff --check
```

For documentation-only changes, check the links and commands you edit and run
`git diff --check`. Execute changed LARP examples through the real compiler and
VM; parsing alone does not establish that a program works.

The [CI workflow](.github/workflows/ci.yml) also runs optimized compiler tests,
GC-stress/artifact/formatted conformance, reference examples, version contracts,
resource measurements, reproducible builds, and candidate compatibility. Run
the [additional checks](docs/development.md#additional-checks) that cover your
change; a passing local baseline does not replace those CI gates.

The current conformance inventory contains **89 active cases**. Its
[inventory checker](tools/check_corpus.py) validates metadata and paths; the
[execution harness](tools/run_conformance.py) compiles and runs cases. A pending
fixture is a design example, not a passing language feature. Update the
[corpus manifest and expectations](tests/conformance/README.md) together when
adding a case.

## License

LARP is licensed under the [MIT License](LICENSE), copyright Hlib Strochkovskyi.
Contributions are made under that license. Dependencies and bundled runtime
components retain their own notices.
