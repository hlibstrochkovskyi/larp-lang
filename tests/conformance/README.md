# Proposed language conformance corpus

Cases in `manifest.json` are either **active** or **pending**. Active cases run
through the real compiler and VM. Pending cases specify future milestones and
must not be reported as passing tests. The current inventory contains 77 cases,
all active, with no pending fixtures. This finite corpus is not a proof that
every normative rule or developer-preview release gate is complete.

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
codes, and presence of source locations. `--all` includes pending cases when
any exist; currently it selects the same 77 active cases and passes. An optional
`--bin PATH` selects an existing binary. `--gc-stress` requests collection at
every object, array, closure, and lazy-cell allocation safepoint; CI executes the
corpus with both normal collection and this stress mode. `--artifacts` adds
`build`/loaded `check`/loaded `run`, comparing exit status, stdout, and stderr
with direct source execution. Invalid source must fail to build without creating
a file. `--formatted` additionally formats temporary copies, checks idempotence,
and compiles/executes the same expectations. It composes with `--artifacts` and
`--gc-stress`; CI exercises the combined path. Error locations must remain valid,
while line numbers may change after formatting. Checked-in files are not modified.

The harness must fail if an unsupported case is silently skipped within a release's
declared subset. Report fixtures pin the current deterministic renderer; its
layout remains an implementation format rather than a frozen language contract.
Incremental milestone suites can stay smaller than the full target corpus.
