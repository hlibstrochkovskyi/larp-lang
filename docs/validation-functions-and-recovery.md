# Functions, recovery, and closure GC validation

Date: 2026-10-17. Environment: Linux, rustc/Cargo 1.99.0. The workspace has no
third-party Rust dependencies and builds offline. This report records local
validation of the implemented subset, not full Specification v0.1 conformance.

## Executed checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 82 tests passed |
| `cargo test -p larp-compiler --release --locked --offline` | 27 integration tests passed |
| `python3 tools/check_corpus.py` | 40 valid fixture definitions: 22 active, 18 pending |
| `python3 tools/run_conformance.py` | 22/22 active cases passed |
| `python3 tools/run_conformance.py --gc-stress` | 22/22 active cases passed |
| `sh tools/git-local.sh diff --check` | Passed |

The workspace suite includes 13 function/recovery integration tests, 21
closure/result-focused bytecode-verifier tests, two closure-heap tests, and seven
CLI integration tests. The release compiler suite includes the existing 14 core
tests and the 13 function/recovery tests. Test functions exercise multiple source
programs and rejection cases; these totals do not count every assertion or every
normal/stress execution as a separate test.

## Compiler and runtime evidence

Positive source programs pass through parsing, semantic analysis, typed
control-flow IR, bytecode lowering, mandatory verification, and the VM. Tests
cover named and prelude functions as values, contextual and annotated lambdas,
immutable snapshot captures, nested/returned closures, lexical shadowing, and
callee-before-argument evaluation. Negative cases reject capture reassignment,
ambiguous local inference, inconsistent returns, wrong arity, function equality,
and effect erasure through higher-order boundaries. A divergence regression
checks that an unreachable return expression does not force an inferred lambda
result to `unit`, and that a never-returning `crashout` receives the expected
`verdict` context in conditions while numeric truthiness still reports `LARP-T002`.

Option/result cases exercise nested payloads, structural equality where allowed,
eager option fallbacks, eager recovery-handler creation, error-only handler
invocation, pure recovery, and function payloads retained inside tagged values.
Parsing checks whole signed ASCII decimal strings and i32 boundaries. Integer
conversions test success, nominal error messages, unsupported source/target pairs,
and the aborting conversion's `LARP-R008` code and exact source span.

The verifier tests crafted in-memory modules independently of the frontend. They
exercise capture layout and order, immutable capture stores in unreachable code,
direct calls to capturing functions, indirect call operands and effects,
malformed intrinsic signatures, invariant container payloads, and typed result
branches and merges. Type-complexity cases include excessive depth, shared type
DAGs, and invalid forward-call/closure targets. A module-wide type preflight runs
before dataflow can compare another function's signature.

## Collection evidence

Function/recovery success programs run with normal collection and collection
before every closure allocation. Their execution statistics assert that all
allocated closure entries are reclaimed after successful termination. A churn
program allocates 2001 closures, preserves a live returned closure, runs at least
2001 stress collections, and keeps fewer than 20 entries live at peak.

Heap tests verify generation reuse and transitive rooting through captured
options. A synthetic 20,002-entry capture graph includes a cycle: it remains live
while rooted and is fully reclaimed after its root disappears. Marking is
iterative. This internal graph tests collector behavior; current source-level
immutable captures cannot construct mutable capture cycles.

The CLI accepts `--gc-stress` together with `--fuel`; argument validation and
instruction exhaustion are tested. The callback example was executed with:

```sh
cargo run -p larp-cli --locked --offline -- run examples/callbacks.larp --gc-stress
```

Its stdout is `20\n98\nvalue does not fit mf\n0\n`.

## Limits of the evidence

M1 and the M2 feature set are implemented. M3 remains in progress: arrays,
indexing, collection pipelines, and callback mutation checks are pending. Classes,
roles, extension resolution, scenario regions, `larp/flex/canon/palantir`, lazy
bindings, persisted artifacts, and formatting remain unimplemented. Function
parameters and ordinary local bindings cannot have type `unit` in this subset.

Tracing currently covers closure storage and closure handles inside owned
option/result payloads. It does not establish correctness for future mutable
object graphs, array allocation, lazy cells, or scenario snapshots. The separate
scenario model is an experiment, not evidence of compiled scenario safety.

The 18 pending fixtures have not passed language conformance. Deterministic
malformed-input and adversarial bytecode tests do not replace sustained fuzzing,
a soundness argument, performance measurement, or a persisted-bytecode loader
audit. CI is configured to run normal and stress conformance, but no hosted run
is claimed and no remote repository has been configured.
