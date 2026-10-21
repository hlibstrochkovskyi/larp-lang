# Multi-file acceptance projects

These projects exercise whole-project compilation in the `0.2.0-preview.1` candidate.
They extend the single-file corpus in `tests/conformance`; they are not features
of the published `v0.1.0-preview.1` release. Each project has one entry unit and
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

## Packaged-binary acceptance

The [manifest](manifest.json) binds the exact source inventory to stdout and
inspection expectations. Run the standalone driver without rebuilding a CLI:

```sh
python3 tools/run_projects.py --bin target/debug/larp --output artifacts/project-validation
```

It executes one project in five modes: ordinary/stress source, ordinary/stress
loaded artifact, and a formatted loaded artifact with stress GC. Formatting is
checked per source unit; source snapshots are renamed and retained before
artifact execution so the original embedded paths are unavailable. The driver
retains commands, results, binary/manifest/source hashes, and a passed/failed
report. Reusing nonempty output is rejected without overwriting prior evidence.

CI runs this driver in its normal check job and against the identical packaged
binary on Ubuntu 24.04 and 26.04. The candidate matrix rejects missing project
execution, missing modes, or mismatched executable/fixture inventory.
