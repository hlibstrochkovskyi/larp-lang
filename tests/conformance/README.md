# Proposed language conformance corpus

Cases in `manifest.json` are either **active** or **pending**. Active cases run
through the real compiler and VM. Pending cases specify future milestones and
must not be reported as passing tests.

`python3 tools/check_corpus.py` validates metadata, paths, diagnostic IDs, and
inventory completeness. It does not parse, type-check, compile, or run LARP.

- `run`: compilation must succeed and stdout must match exactly. Optional
  `stderr` declares exact inspection output; otherwise stderr must be empty.
- `compile_error`: compilation must fail with the specified primary code.
- `runtime_error`: compilation must succeed, then execution must fail with the
  specified runtime code. Optional `stdout` declares observations emitted before
  failure; otherwise stdout must be empty. Host tests also inspect source spans.

`python3 tools/run_conformance.py` builds the CLI and executes every active case
through both `check` and `run`. It checks exit codes, exact stdout/declared inspection output, primary error
codes, and presence of source locations. `--all` also attempts pending cases and
currently fails as expected. An optional `--bin PATH` selects an existing binary.
`--gc-stress` requests collection before every object/array/closure allocation; CI executes
the active corpus both with normal collection and this stress mode.

The harness must fail if an unsupported case is silently skipped within a release's
declared subset. Report fixtures pin the current deterministic renderer; its
layout remains an implementation format rather than a frozen language contract.
Incremental milestone suites can stay smaller than the full target corpus.
