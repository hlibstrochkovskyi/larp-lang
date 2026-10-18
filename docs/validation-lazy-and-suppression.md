# Lazy bindings and output suppression validation

Date: 2026-10-18. Environment: Linux, rustc/Cargo 1.99.0. The dependency-free
workspace builds offline. `nonchalant` and `sybau` are implemented as the first
M6 slice; persisted artifacts, formatting, host APIs, and preview release gates
remain pending. The local validation below has completed.

## Acceptance checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 291 test functions passed |
| `cargo test -p larp-compiler --release --locked --offline` | 117 test functions passed |
| `python3 tools/check_corpus.py` | 77 definitions, all active, none pending |
| `python3 tools/run_conformance.py --all` | 77/77 passed |
| `python3 tools/run_conformance.py --all --gc-stress` | 77/77 passed |
| `sh tools/git-local.sh diff --check` | Passed |

All corpus fixtures are now active, including the original lazy-capture and
lazy-observation cases. `--all` selects the same inventory as the default mode.
Passing this finite corpus is not evidence that every normative rule, resource
behavior, or developer-preview requirement is complete.

The example was executed through the real CLI with:

```sh
cargo run -p larp-cli --locked --offline -- run examples/lazy.larp --gc-stress
```

Its stdout is `49\n49\n`: declaration captures `base = 7`, the later reassignment
does not change the initializer's input, two reads share the memoized square,
and the `sybau` observation is suppressed. Execution passes through semantic
checking, typed IR, bytecode lowering, mandatory verification, and the VM.

## Compiler acceptance coverage

Nineteen new compiler tests cover unused fatal initializers, repeated forcing within an exact
instruction budget, error codes and initializer spans, snapshot capture timing,
nested fixed-array captures, expected result types, and actual effect bounds.
Capturing a lazy binding in another lazy declaration or in an ordinary closure
forces it immediately, so a capture-time abort occurs even when the newly
created computation remains unused. Reference aliases, non-SnapshotValue results,
reference captures, and observation in initializers are rejected.

Constructor cases reject partial-self capture while allowing snapshots of
initialized field values. Regional tests allow locally created snapshot thunks
and specialize pure helper calls into the same region; outside lazy imports are
rejected. Source bindings remain read-only, and internal lazy cells do not become
source-level function results, fields, callback captures, or container payloads.

Suppression tests cover nesting, direct/indirect/role/extension/callback calls,
return and break across nested scopes, inherited caller suppression, and ordinary
observations after leaving the block. Static `observe` effects and constructor
restrictions remain enforced. `palantir`, fatal errors, and inspection-sink
failures remain visible; suppressed `yap` still evaluates its arguments and may
fail during computation before reaching its sink.

## Bytecode and runtime acceptance coverage

`MakeLazy` and `Force` use checked target signatures and internal cell types.
Seven new lazy/suppression bytecode tests and an additional regional-boundary
test independently construct modules. They reject malformed capture/result/effect metadata, cell escapes,
wrong forcing operands, and regional cross-context targets, including forbidden
operations in unreachable code. Silence depth must agree at control-flow joins;
underflow and excessive nesting are rejected. Backend adversarial coverage is
part of the completed validation run above. A new type test checks internal-only
cell shapes and effect invariance; an IR test checks lowering and source spans.

Seven new VM tests independently exercise exact-budget memoization, unused aborting thunks,
a cell retained only by its forcing frame across nested allocations, frame limits,
and cached results that release captured initializer inputs. Reentrant forcing
reports `LARP-R009`; successful forcing records `Ready`, while an active thunk is
`Evaluating`. Collection traces cells and active forcing frames in normal and
stress execution. Source success tests assert matching output and complete heap
reclamation at termination.

Dynamic suppression is saved at call-frame entry and restored at return. Runtime
cases cover nested calls, callers that were already muted, a sink that must not
receive suppressed writes, and fatal failures that remain diagnostic errors.
These paths run in the VM, not a host-language evaluator.

## Limits of the evidence

M6 remains incomplete: no persisted `.larpbc` format or loader, formatter, native
host I/O API, release platform/license selection, or hosted CI evidence is claimed.
Modules and richer language facilities remain deferred. Lazy behavior is
single-threaded and supports SnapshotValue results/captures only. There are no
lazy handles in the public language, retry semantics, asynchronous forcing, or
recoverable initializer failure. Suppression controls ordinary `yap` output; it
is not a general effect handler.

These tests do not establish a soundness proof, sustained fuzzing coverage,
host OOM behavior, total host allocations, or a wall-clock bound for individual
built-ins. Scenario limitations remain: one concrete value-only root, non-nested
trials, checked regional call graphs, and field-type preservation rather than
application-invariant preservation. Historical validation reports record their
own milestone counts and are not rewritten by this slice.
