# Language conformance corpus

[Project overview](../../README.md) · [Specification](../../docs/spec/v0.1.md) · [Contributing](../../CONTRIBUTING.md)

The [manifest](manifest.json) defines executable acceptance cases for the real
LARP toolchain. It currently contains **89 active cases and no pending cases**.
A passing finite corpus provides evidence for these programs; it does not prove
all specification rules or replace the other release checks.

## Run the corpus

Run from the repository root:

```sh
python3 tools/check_corpus.py
python3 tools/run_conformance.py
```

The first command checks metadata, paths, diagnostic IDs, and inventory
completeness. It does not parse, compile, or execute LARP. The second builds the
CLI and checks every active case through `check` and `run`, in text and JSON
diagnostic modes. Programs that compile execute in the VM; compile-error cases
must fail before execution.

Additional execution paths use the same expectations:

```sh
python3 tools/run_conformance.py --gc-stress
python3 tools/run_conformance.py --artifacts
python3 tools/run_conformance.py --artifacts --gc-stress --formatted
```

| Option | What it checks |
| --- | --- |
| `--bin PATH` | Use an existing CLI without rebuilding it |
| `--gc-stress` | Collect at every object, array, closure, and lazy-cell allocation safepoint |
| `--artifacts` | Build bytecode, load/verify it, and compare execution with direct source execution |
| `--formatted` | Format temporary copies, check idempotence, and compile/execute the same contracts |
| `--all` | Include pending cases as well as active ones; pending failures are expected |

CI exercises normal, stress-GC, artifact, and combined formatted/artifact/stress
paths. Checked-in source files are never reformatted by this harness. Formatted
copies may have different line numbers; JSON ranges are derived independently
from their actual source bytes. Invalid source must fail to build without
creating an artifact.

## Case contracts

Cases have an ID, source path, milestone, status, and expectation. Every `.larp`
file in this directory must appear exactly once in the manifest. **Active**
cases are required to pass; **pending** cases describe future milestones and
must not be reported as passing tests or silently skipped within a release's
declared subset. With the present inventory, `--all` selects the same 89 cases.

| Expectation | Required behavior |
| --- | --- |
| `run` | Compilation succeeds; exit status is 0 and stdout matches exactly |
| `compile_error` | Compilation fails with the declared primary diagnostic code |
| `runtime_error` | Compilation succeeds; VM execution fails with the declared runtime code and source span |

For `run`, optional `stderr` is exact inspection output; otherwise stderr must
be empty. Runtime-error cases can declare `stdout` and `stderr` emitted before
failure; omitted values mean empty output. Their required `span` identifies a
nonempty source slice appearing exactly once. The harness independently computes
its UTF-8 byte offsets and scalar coordinates and checks the full primary label
through the [JSON diagnostic protocol](../../docs/diagnostics.md).

JSON execution must preserve stdout and exit status. Inspection events must
reconstruct the exact declared report; a fatal diagnostic must be the last
stderr event. Text-mode errors must include source locations. Report fixtures
pin the current deterministic inspection renderer, whose layout is an
implementation format rather than a frozen language contract.

## Host input

Manifest schema version 3 permits an optional `input` object containing UTF-8
`stdin` text and/or an `args` array of UTF-8 strings. NUL is forbidden in OS
arguments. Run/runtime-error cases may declare input; absent stdin is explicit
EOF and absent arguments is an empty list. Compile-error cases cannot consume
host input.
