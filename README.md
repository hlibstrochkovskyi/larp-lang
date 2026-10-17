# LARP

*Fake it till you compile.*

LARP is a language project: familiar C-style syntax, strict static types,
nominal objects, and controlled what-if execution. The jokes belong in the
vocabulary; correctness belongs in the compiler.

```larp
main_character {
    unc money = fortnine;
    locked_in while (money < 1000) {
        money += 10;
    }
    yap(money);
}
```

**Status: executable algorithmic core with classes, collections, and closures.**
The example above compiles to verified in-memory bytecode and runs on the LARP VM, printing
`1009`. Functions, recursion, loops, checked arithmetic, effect bounds, first-class
functions, snapshot captures, options/results, checked conversions, dynamic/fixed
arrays, and eager collection pipelines work today. Nominal classes support
constructors, public/private fields and methods, and bound methods. Objects,
closures, and dynamic arrays use a tracing collector. Roles, extensions, and
language-level scenarios remain in development; the separate scenario model is
still an experiment.

## Start here

- [Design review](docs/design-review.md): strengths, contradictions, and proposed decisions.
- [Language Specification v0.1](docs/spec/v0.1.md): proposed normative semantics.
- [Grammar](docs/spec/grammar.ebnf): syntax for the proposed subset.
- [Compiler architecture](docs/architecture.md): pipeline, memory, and runtime boundaries.
- [Roadmap](docs/roadmap.md): milestones with acceptance gates.
- [Implementation status](docs/implementation-status.md): supported features and limits.
- [Research](docs/research.md): primary sources and their limits.
- [Scenario evaluation](docs/scenario-evaluation.md): practical cases and a counterexample.
- [Development](CONTRIBUTING.md): local setup and checks.
- [Git setup](docs/git-setup.md): saved history in this managed workspace.
- [Conformance corpus](tests/conformance/README.md): required programs and failures.

## Local development

Use Rust 1.99.0 or later, Cargo, rustfmt, Clippy, and Python 3.11 or later.
The workspace currently has no third-party Rust dependencies and builds offline.

```sh
cargo run -p larp-cli -- --help
cargo run -p larp-cli -- explain LARP-T041
cargo run -p larp-cli -- run examples/primes.larp
cargo run -p larp-cli -- run examples/callbacks.larp --gc-stress
cargo run -p larp-cli -- run examples/collections.larp --gc-stress
cargo run -p larp-cli -- run examples/objects.larp --gc-stress
cargo run -p larp-cli -- dump examples/primes.larp
cargo test --workspace --locked --offline
python3 tools/run_conformance.py
python3 tools/run_conformance.py --gc-stress
```

The implemented CLI commands are `--help`, `--version`, `explain CODE`,
`check FILE`, `run FILE [--fuel N] [--gc-stress]`, and `dump FILE [--ir]`.
`build` and `fmt` report that they are unavailable. Bytecode currently lives
in memory; there is no persisted `.larpbc` format yet.

The compiler implementation direction is Rust → typed IR → bytecode → LARP VM.
The language semantics do not depend on Rust, LLVM, or any particular collector.
Specification v0.1 defines the broader target; implementation coverage is tracked
separately and must not be inferred from syntax in the design examples.

No remote or distribution license has been selected. Private design material is
excluded from version control.
