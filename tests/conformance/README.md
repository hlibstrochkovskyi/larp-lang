# Proposed language conformance corpus

Every case in `manifest.json` is **pending**. No LARP compiler exists yet.
These files are executable requirements for future milestones, not passing tests.

`python3 tools/check_corpus.py` validates metadata, paths, diagnostic IDs, and
inventory completeness. It does not parse, type-check, compile, or run LARP.

- `run`: compilation must succeed and stdout must match exactly.
- `compile_error`: compilation must fail with the specified primary code.
- `runtime_error`: compilation must succeed, then execution must fail with the
  specified runtime code. Host test infrastructure will also inspect source spans.

The future harness must invoke the real CLI/compiler/VM and fail if an unsupported
case is silently skipped within a release's declared subset. Stderr report layout
is not yet a byte-for-byte contract; scenario field semantics are specified.
Incremental milestone suites can stay smaller than the full target corpus.
