# File-module validation

Date: 2026-10-20. Scope: the `0.2.0-dev` file-module implementation based on
`main` commit `5bd3f1f4f9de9be6b7ce6d195bf952f21d88bdc1`.
This records local acceptance of [RFC 0001](rfcs/0001-file-modules.md), not a new
published release. Each subsequent commit still requires its own hosted CI.

## Implemented path

UTF-8 source units pass through the real lexer/parser, injected-resolver graph,
lexical symbol/type/effect analysis, typed CFG IR, bytecode lowering, independent
verification, and iterative VM. CLI imports use canonical contained regular files.
The compiler library performs no filesystem lookup. Execution needs no resolver;
whole-project artifacts embed the original source maps in existing wire version 2.

## Acceptance evidence

| Contract | Executed evidence |
| --- | --- |
| Unit grammar and retained spans | Seven syntax source-unit tests: imports, qualifiers, visibility, missing/duplicate entry placement, original source slices, and legacy grammar separation |
| Stable graph identity and bounded loading | Seventeen graph tests: breadth-first IDs, diamonds, duplicate aliases, self/multi-file cycles, wrong entries, inconsistent resolver snapshots, UTF-8 bytes, line/module/edge boundaries |
| Cross-file language semantics | Thirty-eight compiler project tests: nominal classes/roles, private helpers, constructors/members, callbacks, closures, lazy bodies, extensions, errors, and scenarios |
| Exported interfaces | Recursive `M006` failures for private nominal types in public parameters/results, constructors, fields/methods, roles, nested containers, and function types; private implementation fields remain valid |
| Explicit extensions | Exact numeric/role receivers, lexical dot lookup, real-member priority, one receiver evaluation, contextual remaining arguments, first-class selection/effect widening, private overload exclusion, and `M007` collision/ambiguity |
| Numeric result context | A reproduced regression for `3_000_000_000 + ext::identity(wide)` now passes in both operand orders and through encoded/decoded artifacts |
| Filesystem policy | Eight provider tests: canonical snapshots, aliases, normalized relative paths, containment/symlinks, extension/UTF-8/read limits, missing and nonregular sources |
| CLI and persisted projects | Twelve project integration tests: check/run/dump/build/fmt, loaded artifacts after temporary sources are removed, ordinary/stress GC, exact outputs, and imported-source overwrite prevention |
| Multi-source diagnostics | Actual imported runtime errors retain exact JSON primary spans after source removal; cycle errors retain exact primary/related ranges and paths. A separate loaded-source-map rendering test checks cross-file related labels after deletion without claiming VM stack traces |
| Formatting | Seven source-unit formatter tests plus CLI checks for raw tokens/comments, compact qualifiers, library units, original error spans, reparse, and idempotence |

Positive compiler project cases using the shared execution helper run both the
directly compiled and independently encoded/decoded module, each normally and
with stress GC. Assertions cover output, inspection where applicable, and
collector reclamation. Dedicated scenario tests and the committed
[pitch project](../tests/projects/README.md) exercise performed state and selective
commit. The pitch CLI test checks `49`, `20`, `fr` and its exact `PALANTIR` report,
then repeats execution from an artifact after removing only temporary source copies.

## Local checks

All of the following passed on this implementation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo test -p larp-compiler --release --locked --offline
python3 tools/check_corpus.py
python3 tools/run_conformance.py
python3 tools/run_conformance.py --gc-stress
python3 tools/run_conformance.py --artifacts --gc-stress
python3 tools/run_conformance.py --formatted --artifacts --gc-stress
python3 tools/run_reference.py
python3 tools/run_reference.py --document README.md --contracts docs/readme-cases.json
python3 tools/check_version.py --bin target/debug/larp
python3 -m unittest discover -s tools -p 'test_candidate_checks.py'
git diff --check
```

Results: **480 workspace tests**, **193 optimized compiler tests**, **17 release-tool
tests**, and **89/89 unchanged single-file conformance cases in each of four modes**.
The reference harness executed eight samples through 40 VM runs and 113 CLI
commands; the README contract added five VM runs and 15 commands. The new module
guide's two source units also executed with exact `42` output normally and under
stress GC. Version checks executed historical artifact v1/v2 and confirmed writer
2, readers 1/2, and unchanged JSON schemas under the development version.

The separate fuzz workspace's locked offline metadata resolved after mechanically
updating local package versions; registry entries/checksums, targets, seeds, and
campaign settings were unchanged. This does not establish fuzz coverage of the
new graph/provider APIs. The established hosted workflow runs independently.
Reproducible packaging and Ubuntu candidate compatibility are hosted CI gates,
not inferred from the local unit/conformance results above.

## Limits

The local gate covers this bounded file-module design, not packages, re-exports,
cyclic imports, globals/initializers, separate compilation, dynamic loading, or
network discovery. Existing effects, source/artifact budgets, value-only scenario
roots, and non-nested scenarios still apply. Canonical path containment is not
protection against concurrent hostile filesystem mutation. No source stability,
sandbox, universal platform, or whole-specification correctness claim is made.

The original root checkout's unfinished `feature/modules` files are preserved;
this implementation uses the separate `feature/file-modules` worktree. The
published preview tag and archive remain unchanged.
