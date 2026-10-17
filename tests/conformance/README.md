# Proposed language conformance corpus

Cases in `manifest.json` are either **active** or **pending**. Active cases run
through the real compiler and VM. Pending cases specify future milestones and
must not be reported as passing tests.

`python3 tools/check_corpus.py` validates metadata, paths, diagnostic IDs, and
inventory completeness. It does not parse, type-check, compile, or run LARP.

- `run`: compilation must succeed and stdout must match exactly.
- `compile_error`: compilation must fail with the specified primary code.
- `runtime_error`: compilation must succeed, then execution must fail with the
  specified runtime code. Host test infrastructure will also inspect source spans.

`python3 tools/run_conformance.py` builds the CLI and executes every active case
through both `check` and `run`. It checks exit codes, exact stdout, primary error
codes, and presence of source locations. `--all` also attempts pending cases and
currently fails as expected. An optional `--bin PATH` selects an existing binary.

The harness must fail if an unsupported case is silently skipped within a release's
declared subset. Stderr report layout
is not yet a byte-for-byte contract; scenario field semantics are specified.
Incremental milestone suites can stay smaller than the full target corpus.
