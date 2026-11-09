<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/LARP%20Programming%20Language%20Doodle%20Logo.png">
    <source media="(prefers-color-scheme: light)" srcset="docs/LARP%20Programming%20Language%20Logo.png">
    <img src="docs/LARP%20Programming%20Language%20Logo.png" alt="LARP programming language logo" width="720">
  </picture>
</p>

# LARP

*Fake it till you compile.*

[![CI](https://github.com/hlibstrochkovskyi/larp-lang/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hlibstrochkovskyi/larp-lang/actions/workflows/ci.yml)
[![Preview](https://img.shields.io/badge/preview-0.2.0--preview.1-8b5cf6)](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.2.0-preview.1)
[![MIT](https://img.shields.io/badge/license-MIT-8b5cf6)](LICENSE)

[Download](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.2.0-preview.1) · [Learn](docs/reference.md) · [Documentation](docs/README.md) · [Contribute](CONTRIBUTING.md)

LARP is an experimental, statically typed programming language with C-style
syntax, objects, first-class functions, and controlled what-if execution.
Written in Rust, it compiles to its own verified bytecode and runs on the LARP VM.

**[v0.2.0-preview.1](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.2.0-preview.1)
is out: File Modules.** Split programs into libraries, export declarations with
`highkey`, and call them through `alias::name`. The whole project still compiles
to one verified bytecode artifact. [Learn modules →](docs/modules.md)

## Pitch the unicorn. Keep the aura.

No funding? `.copium()` has you covered. Farm some aura, rehearse the billion-dollar
valuation, and decide which changes become real.

```larp
archetype Founder {
    highkey unc valuation;
    highkey mf aura;

    init(unc seed) {
        self.valuation = seed;
        self.aura = 0;
    }

    highkey sidequest isMogging(mf rival) -> verdict uses(read) {
        return self.aura mogg rival;
    }
}

main_character {
    baddie<unc> funding = no_bitches;
    Founder bro = new Founder(funding.copium(fortnine));

    mfs<mf> clips = [2, 4, 49];
    mf qualified = clips.map(x => x + 1).filter(x => x mogg 4).size();

    larp (bro) as UnicornPitch {
        nonchalant mf pitchDeckAura = qualified * 10;
        flex bro.valuation = 1_000_000_000;
        bro.aura += pitchDeckAura;
        canon bro.aura;
        palantir(bro);
    }

    yap(bro.valuation);
    yap(bro.aura);
    yap(bro.isMogging(10));
}
```

The console prints `49`, `20`, and `fr` on separate lines. `palantir` separately
reports the trial on stderr:

```text
PALANTIR [UnicornPitch]
PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED
valuation | 49 | 1000000000 | 1000000000 | false
aura | 0 | 20 | - | true
```

The valuation was a performance. The aura is canon.

This is a [complete runnable example](examples/pitch-deck.larp), checked against
the released compiler and in CI. It combines objects, explicit method effects,
optional values, collection callbacks, memoized computation, and selective commit.
Scenario roots currently need value-only fields; selected fields keep their final
trial values on normal block completion. This preserves field types, not
relationships between fields. See [scenario semantics and limits](docs/scenario-evaluation.md).

## Download and run

**[v0.2.0-preview.1](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.2.0-preview.1)**
is available for Linux x86-64 GNU, tested on Ubuntu 24.04 and 26.04.
The archive includes the CLI, source, documentation, examples, and license notices.
Running the prebuilt CLI requires no Rust installation.

Download the `.tar.gz` and its matching `.sha256` from the release, or use:

```sh
larp_release=https://github.com/hlibstrochkovskyi/larp-lang/releases/download/v0.2.0-preview.1
larp_archive=larp-0.2.0-preview.1-x86_64-unknown-linux-gnu-a053c991d298.tar.gz
curl -fLO "$larp_release/$larp_archive"
curl -fLO "$larp_release/$larp_archive.sha256"
sha256sum -c "$larp_archive.sha256"
tar -xzf "$larp_archive"
cd "${larp_archive%.tar.gz}"
./bin/larp run examples/pitch-deck.larp
./bin/larp run tests/projects/pitch/main.larp --gc-stress
```

Both pitch programs are included in this archive. The multi-file version calls
an exported extension inside a scenario and commits only the earned aura.
See the [publication record](docs/releases/0.2.0-preview.1-publication.md) for the
exact commit, checksums, and compatibility evidence. The [first preview](docs/releases/0.1.0-preview.1-publication.md)
remains available as a historical release.

### Build from source

Use Rust **1.99.0+**, Cargo, and a native linker. The manifest parser and its
locked dependencies are vendored, so workspace builds work offline after
installing the toolchain. See [third-party sources and licenses](THIRD_PARTY.md).
Python **3.11+**, rustfmt, and Clippy are needed for development checks.

```sh
git clone https://github.com/hlibstrochkovskyi/larp-lang.git
cd larp-lang
cargo build -p larp-cli --release --locked --offline
./target/release/larp run examples/pitch-deck.larp
```

Source builds use the checked-out commit; they do not reproduce the published
archive merely by sharing its version string. This development branch reports
`0.3.0-dev` and adds the [project workflow](docs/projects.md); the published
`0.2.0-preview.1` does not include that workflow. Other platforms are not yet part
of the release compatibility policy.

### Start a project on the development branch

With a `0.3.0-dev` source build available on `PATH`:

```sh
larp new aura-app
cd aura-app
larp check
larp run --gc-stress
larp build
larp run target/aura-app.larpbc
```

The generated two-file program prints `Your first sidequest:`, `50`, and `fr`.
`larp.toml` selects the entry source; `check`, `run`, and `build` discover it from
the project directory or its descendants. Explicit file commands continue to
work. See [project commands and boundaries](docs/projects.md) before adapting an
existing directory. This is development functionality, not a new public release.

## Cursed vocabulary. Concrete semantics.

| LARP | What it does |
| --- | --- |
| `import`, `highkey`, `alias::name` | Explicit file modules, exports, and lexical namespaces with checked interfaces |
| `mf`, `unc`, `geeked`, `verdict` | Exact i32, i64, f64, and Boolean types; no implicit truthiness or numeric conversions |
| `archetype`, `role`, `sidequest` | Nominal objects, interfaces, and functions with checked signatures and effect bounds |
| `mfs<T>`, `hediJeans<T,N>` | Shared dynamic arrays and fixed-size value arrays, with checked indexing |
| `baddie<T>`, `result<T,E>` | Explicit absence and expected errors, with `.copium()` and `.cope()` recovery |
| `larp`, `flex`, `canon`, `palantir` | Isolated value-state trials, explicit claims, selective commit, and inspection |
| `nonchalant`, `sybau` | Pure memoized computation and scoped suppression of ordinary `yap` output |

Closures capture bindings as snapshots. A tracing collector manages objects,
arrays, closures, and lazy cells. Source checks and an independent bytecode
verifier preserve types, visibility, effects, and scenario restrictions.

The published preview supports standalone and multi-file programs; development
builds also support manifest-based projects. Dependency packages, re-exports,
user-defined generics, inheritance, async, FFI, and a native backend remain future
work. There is no stable source/API compatibility promise or hostile-code sandbox.
The [implementation status](docs/implementation-status.md) lists working features
and precise limits; the [specification](docs/spec/v0.1.md) remains a design baseline.

## Explore the toolchain

After a source build:

```sh
./target/release/larp --version --json
./target/release/larp check examples/pitch-deck.larp
./target/release/larp run tests/projects/pitch/main.larp
./target/release/larp fmt examples/pitch-deck.larp --stdout
trial_dir=$(mktemp -d)
./target/release/larp build examples/pitch-deck.larp -o "$trial_dir/pitch.larpbc"
./target/release/larp run "$trial_dir/pitch.larpbc" --gc-stress
./target/release/larp explain LARP-T041
```

`fmt --stdout` previews formatting; `fmt --check` checks it; `fmt FILE` writes it.
Artifacts embed source maps and pass verification when loaded. JSON diagnostics
keep program output on stdout and report structured events on stderr.

## Learn, build, contribute

- **Write LARP:** [practical reference](docs/reference.md), [examples](examples/),
  [project workflow](docs/projects.md), [CLI and diagnostics](docs/diagnostics.md).
- **Understand the implementation:** [architecture](docs/architecture.md),
  [development guide](docs/development.md), [roadmap](docs/roadmap.md).
- **Help the project:** [contribution guide](CONTRIBUTING.md),
  [report a bug](https://github.com/hlibstrochkovskyi/larp-lang/issues),
  [review an open pull request](https://github.com/hlibstrochkovskyi/larp-lang/pulls).
- **Go deeper:** [documentation index](docs/README.md), including language design,
  execution contracts, validation reports, and distribution policy.

## License

[MIT](LICENSE) · Copyright (c) 2026 **Hlib Strochkovskyi**.
Vendored dependencies and packaged Rust runtime components retain their own
license notices; see [THIRD_PARTY.md](THIRD_PARTY.md).
