# Multi-file acceptance projects

These projects exercise whole-project compilation with the development toolchain.
They extend the single-file corpus in `tests/conformance`. The cross-file pitch
is supported by the published `v0.2.0-preview.1`; the enum/result project requires
`0.3.0-dev` and is not part of that published release. Each project has one entry unit and
explicitly imported library units. Library files intentionally have no
`main_character` block and cannot run as independent programs.

## Cross-file pitch

`pitch/main.larp` combines exported nominal objects, a concrete extension called
through a module qualifier, a collection pipeline with a private named callback,
and isolated scenario execution with selective commit. The source files are:

- `pitch/main.larp`: imports libraries, constructs a founder, computes earned aura,
  calls `founders::train(bro, earned)` inside a scenario, and selects only aura.
- `pitch/lib/founder.larp`: exports `Founder` and the `Founder.train` extension.
  The extension's `self` receiver is the first argument of the qualified call.
- `pitch/lib/training.larp`: exports a collection operation; its `boost` helper
  stays private to its defining file.

Run from the repository root with the development toolchain:

```sh
cargo run --locked --offline -- run tests/projects/pitch/main.larp
cargo run --locked --offline -- run tests/projects/pitch/main.larp --gc-stress
cargo run --locked --offline -- build tests/projects/pitch/main.larp -o /tmp/larp-pitch.larpbc
cargo run --locked --offline -- run /tmp/larp-pitch.larpbc
```

Standard output is exactly:

```text
49
20
fr
```

The inspection on standard error occurs before scenario commit:

```text
PALANTIR [CrossFilePitch]
PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED
valuation | 49 | 1000000000 | 1000000000 | false
aura | 0 | 20 | - | true
```

The claimed valuation is discarded; the selected aura survives normal scenario
completion. The public `isMogging` method then observes the committed aura.

`crates/larp-cli/tests/projects.rs` copies these exact tracked source files into a
fresh temporary project. It checks formatting idempotence per unit, checks and
executes the project normally and under stress GC, builds an artifact, removes
only the temporary source copies, and checks and runs that artifact in both GC
modes. Assertions cover the exact standard output and inspection above. Run the
acceptance test with:

```sh
cargo test -p larp-cli --test projects --locked --offline committed_pitch_project
```

## Cross-file alternatives and propagation

`sums/main.larp` imports the public `Funding` enum and `parseFunding` function
from `sums/lib/funding.larp`. It extracts a module-qualified variant constructor
as a first-class function, matches every qualified variant, and performs a nested
match over a returned result and its enum payload. The imported function uses
`?` to propagate parse failures before wrapping a successful amount. Its exact
standard output is `20\n49\ninvalid mf\n`, with no standard error.

```sh
cargo run --locked --offline -- run tests/projects/sums/main.larp --gc-stress
```

The standalone driver below checks this project alongside the pitch, including
formatted source and loaded bytecode after source paths become unavailable.

## Packaged-binary acceptance

The [manifest](manifest.json) binds the exact source inventory to stdout and
inspection expectations. Run the standalone driver without rebuilding a CLI:

```sh
python3 tools/run_projects.py --bin target/debug/larp --output artifacts/project-validation
```

It executes each listed project in five modes: ordinary/stress source, ordinary/stress
loaded artifact, and a formatted loaded artifact with stress GC. Formatting is
checked per source unit; source snapshots are renamed and retained before
artifact execution so the original embedded paths are unavailable. The driver
retains commands, results, binary/manifest/source hashes, and a passed/failed
report. Reusing nonempty output is rejected without overwriting prior evidence.

CI runs this driver in its normal check job and against the identical packaged
binary on Ubuntu 24.04 and 26.04. The candidate matrix rejects missing project
execution, missing modes, or mismatched executable/fixture inventory.
