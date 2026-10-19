<p align="center">
  <img src="docs/LARP%20Programming%20Language%20Logo.png" alt="LARP programming language logo" width="720">
</p>

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

**Status: executable core with objects, roles, collections, and isolated scenarios.**
The example above compiles to verified bytecode and runs on the LARP VM, printing
`1009`. Functions, recursion, loops, checked arithmetic, effect bounds, first-class
functions, snapshot captures, options/results, checked conversions, dynamic/fixed
arrays, and eager collection pipelines work today. Nominal classes support
constructors, public/private fields and methods, and bound methods. Objects,
closures, and dynamic arrays use a tracing collector. Nominal role dispatch,
concrete receiver extensions, and built-in method extraction preserve identity
and checked effect bounds. Language-level `larp` blocks now compile to isolated
scenario frames with checked call graphs, selective final-field commit, and
`palantir` reports. Read-only `nonchalant` bindings memoize snapshot computations;
`sybau` suppresses ordinary `yap` output across calls. The separate state model
remains an independent experiment. Versioned `.larpbc` artifacts embed source maps
and pass verification again on loading. The syntax-aware formatter preserves
comments, original token spellings, and program order. `args()` and `readLine()`
provide explicitly effect-checked CLI input through host capabilities.

## Start here

- [Design review](docs/design-review.md): strengths, contradictions, and proposed decisions.
- [Language Specification v0.1](docs/spec/v0.1.md): proposed normative semantics.
- [Grammar](docs/spec/grammar.ebnf): syntax for the proposed subset.
- [Compiler architecture](docs/architecture.md): pipeline, memory, and runtime boundaries.
- [Roadmap](docs/roadmap.md): milestones with acceptance gates.
- [Implementation status](docs/implementation-status.md): supported features and limits.
- [Preview readiness](docs/preview-readiness.md): requirement evidence and open semantic/release gates.
- [First preview candidate](docs/releases/0.1.0-preview.1.md): declared subset, commands, formats, and known limits.
- [Diagnostic protocol](docs/diagnostics.md): JSON Lines, exact source spans, and inspection events.
- [Practical reference](docs/reference.md): implemented syntax and executable examples.
- [Research](docs/research.md): primary sources and their limits.
- [Scenario evaluation](docs/scenario-evaluation.md): executed comparisons and a counterexample.
- [Scenario validation](docs/validation-scenarios.md): compiled scenario acceptance evidence.
- [Lazy and suppression validation](docs/validation-lazy-and-suppression.md): current implementation checks.
- [Host input](docs/host-input.md): arguments, stdin, EOF/errors, and embedding.
- [Source formatter](docs/formatter.md): layout, preservation, and CLI modes.
- [Fuzzing](fuzz/README.md): coverage-guided targets and stable regression replay.
- [Resource measurements](docs/validation-resources.md): VM counters, process memory, and limit witnesses.
- [Reproducible builds](docs/reproducible-builds.md): two clean release builds and explicit limits.
- [Candidate archives](docs/distribution.md): prebuilt CLI, complete source, and extraction checks.
- [Preview platforms](docs/platforms.md): pinned build host and same-archive execution gates.
- [Versioning](docs/versioning.md): toolchain versions, machine-readable identity, and independent format contracts.
- [Bytecode artifacts](docs/bytecode-format-v1.md): baseline wire format and limits; [version 2](docs/bytecode-format-v2.md) adds host input.
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
cargo run -p larp-cli -- run examples/roles.larp --gc-stress
cargo run -p larp-cli -- run examples/lazy.larp --gc-stress
cargo run -p larp-cli -- run tests/conformance/pass/scenario.larp --gc-stress
cargo run -p larp-cli -- dump examples/primes.larp
cargo run -p larp-cli -- build examples/objects.larp -o /tmp/objects.larpbc
cargo run -p larp-cli -- run /tmp/objects.larpbc --gc-stress
cargo run -p larp-cli -- fmt examples/objects.larp --stdout
cargo run -p larp-cli -- run examples/input-sum.larp -- total < scores.txt
cargo test --workspace --locked --offline
python3 tools/run_conformance.py
python3 tools/run_conformance.py --gc-stress
python3 tools/run_conformance.py --artifacts --gc-stress
python3 tools/run_conformance.py --formatted --artifacts --gc-stress
```

The implemented CLI commands are `--help`, `--version`, `explain CODE`,
`build SOURCE [-o OUTPUT]`, `check FILE`, `run FILE [--fuel N] [--gc-stress] [-- ARG...]`,
`dump FILE [--ir]`, and `fmt SOURCE [--check|--stdout]`. Source and artifact input
are supported by `check`, `run`, and bytecode `dump`; `--ir` requires source. Artifacts contain full source text
for diagnostics and run without the original source file. `fmt` updates source
by default; `--stdout` previews and `--check` verifies formatting without changes.

The compiler implementation direction is Rust → typed IR → bytecode → LARP VM.
The language semantics do not depend on Rust, LLVM, or any particular collector.
All 89 current conformance fixtures are active; the corpus is a finite acceptance
suite, not a proof that every language rule is implemented. Specification v0.1
defines the broader target, and M6 developer-preview gates remain open.

The project repository is [hlibstrochkovskyi/larp-lang](https://github.com/hlibstrochkovskyi/larp-lang).
LARP is licensed under the [MIT License](LICENSE). Private design material is
excluded from version control.
