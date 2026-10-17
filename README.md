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

**Status: design and engineering foundation.** There is no LARP parser, type
checker, bytecode compiler, or VM yet. The example above is a target program,
not an executable demonstration. The Rust workspace currently provides source
locations, a diagnostic registry, a small CLI, and a separate executable model
of selective state commit.

## Start here

- [Design review](docs/design-review.md): strengths, contradictions, and proposed decisions.
- [Language Specification v0.1](docs/spec/v0.1.md): proposed normative semantics.
- [Grammar](docs/spec/grammar.ebnf): syntax for the proposed subset.
- [Compiler architecture](docs/architecture.md): pipeline, memory, and runtime boundaries.
- [Roadmap](docs/roadmap.md): milestones with acceptance gates.
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
cargo test --workspace --locked --offline
python3 tools/check_corpus.py
```

The implemented CLI commands are `--help`, `--version`, and `explain CODE`.
`check`, `build`, `run`, and `fmt` are planned; they explicitly report that they
are unavailable. No command pretends to compile a file.

The compiler implementation direction is Rust → typed IR → bytecode → LARP VM.
The language semantics do not depend on Rust, LLVM, or any particular collector.
Specification v0.1 remains a proposal pending maintainer review and experiments.

No remote or distribution license has been selected. Private design material is
excluded from version control.
