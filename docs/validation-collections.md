# Collections and managed array validation

Date: 2026-10-17. Environment: Linux, rustc/Cargo 1.99.0. The dependency-free
workspace builds offline. This report records local evidence for the implemented
M3 feature set; it does not claim full Specification v0.1 conformance.

## Executed checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 118 tests passed |
| `cargo test -p larp-compiler --release --locked --offline` | 41 integration tests passed |
| `python3 tools/check_corpus.py` | 48 valid fixture definitions: 33 active, 15 pending |
| `python3 tools/run_conformance.py` | 33/33 active cases passed |
| `python3 tools/run_conformance.py --gc-stress` | 33/33 active cases passed |
| `sh tools/git-local.sh diff --check` | Passed |

The workspace suite includes 14 compiler collection integration tests, eight VM
collection tests, eight array-focused bytecode-verifier tests, four heap tests,
and nine frontend tests. The release compiler suite includes 14 scalar/core,
13 function/recovery, and 14 collection tests. A test function can exercise many
programs or malformed modules; normal/stress repetitions and individual
assertions are not counted as separate tests.

The example was also executed with:

```sh
cargo run -p larp-cli --locked --offline -- run examples/collections.larp --gc-stress
```

Its stdout is `5\n50\n`. `filter`, `map`, `sorted`, and `forEach` run through
semantic checking, typed control-flow IR, bytecode lowering, mandatory
verification, and the VM.

## Source-language evidence

Collection integration tests cover contextual and inferred dynamic arrays,
contextual fixed arrays including length zero, nested values, checked indexing,
and indexed/compound assignment. Dynamic arrays retain identity through aliases,
calls, closures, and option payloads. Fixed arrays copy their values through
parameters, captures, nested arrays, and tagged values; reference elements still
share identity. Recursive fixed-array equality is accepted only when element
equality is supported. Dynamic-array and function equality remain rejected.

Pipelines execute eagerly and left-to-right, allocate independent results, infer
callback parameter/result types, and use expected output types for ambiguous
option and empty-array payloads. Tests cover integer/text sorting without
changing the input and Unicode-scalar text length, including a combining sequence.
Receiver evaluation precedes callback creation. Indexed replacement during
iteration is permitted and subsequent callbacks see the replacement. Nested
read-only iteration releases each guard; structural mutation through an alias
fails with `LARP-R005` before `push` modifies the receiver.

Assignment regressions save each index once and read the previous compound
value before the right side. Rebuilding fixed-array ancestors preserves a sibling
update made by the right side. Invalid assignment indices fail with `LARP-R004`
at the indexed place before a right-side observation or array growth occurs;
growth cannot retroactively validate the place. Negative/out-of-range index and
iteration-guard diagnostics retain the expected source spans.

Effect cases distinguish fresh allocation, fixed-value mutation, dynamic reads,
dynamic writes, checked indexing/growth, and callback effects. Higher-order
collection operations cannot hide observation or abort effects. Rejection cases
also cover incorrect lengths, heterogeneous elements, unsupported indices,
non-sortable types, invalid callback arity/results, empty-array inference,
captured fixed-value writes, and excessive inferred type complexity.

## Bytecode and runtime evidence

Adversarial in-memory modules test exact intrinsic signatures, counts, receiver
types, index widths, payload types, fixed-update results, callback contracts,
container invariance, and caller/callback effect bounds. Structurally invalid or
effect-forbidden intrinsics are rejected even in unreachable code. Embedded
array/callback types undergo complexity preflight before comparisons or
diagnostic formatting.

The VM tests construct verified modules independently of source compilation.
They exercise array identity versus fixed copying, map outputs that contain
closures, nested iteration and guard release, aliased `push` rejection, permitted
element replacement, index failures, and assignment-place checking. Recursive
higher-order calls hit the configured LARP frame limit rather than recursing
through the host stack.

Successful compiler/VM collection programs run with normal collection and
collection at every allocation safepoint, asserting matching output and complete
reclamation after termination. A source program creates 500 mutable array/closure
cycles while retaining closures produced by `map`: stress collection preserves
their output `1\n2\n3\n`, performs more than 1000 collections, keeps fewer than
30 heap entries live at peak, and reclaims every allocated entry by exit.

Heap tests additionally cover generation reuse, live cycles, wrapper-contained
handles, and deep graph traversal without recursive marking/sweeping. Suspended
callback receivers, callbacks, current filter elements, and unfinished output
buffers are explicit roots. These tests extend the earlier closure evidence to
mutable array/closure graphs.

## Limits of the evidence

M1, M2, and the M3 feature set are implemented. Classes, roles, extensions,
bound-method extraction, scenario regions, `larp/flex/canon/palantir`, lazy
bindings, `sybau`, persisted bytecode, modules, host I/O APIs, and formatting are
still pending. Function parameters, ordinary local bindings, and array elements
cannot have type `unit`. User-defined generic declarations are deferred.

The 15 pending fixtures are not passing language programs. Runtime tests do not
allocate arrays near the maximum `mf` length or prove host OOM behavior. An
instruction budget bounds dispatched bytecode, not wall-clock cost of an
individual built-in operation such as sorting. Stress collection and adversarial
tests provide evidence, not a complete soundness proof, sustained fuzzing result,
or performance guarantee. Object, lazy-cell, and scenario roots need their own
future GC coverage. The independent scenario model does not validate compiled
scenario safety.

CI is configured for these checks; this report makes no claim of a hosted CI
run. No remote repository or distribution license has been selected.
