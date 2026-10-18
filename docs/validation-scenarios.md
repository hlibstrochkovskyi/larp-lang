# Compiled scenarios validation

Date: 2026-10-18. Environment: Linux, Rust/Cargo 1.99.0. The dependency-free
workspace builds offline. M5's declared value-only scenario feature set runs
through the real compiler, typed IR, verified bytecode, and VM. The independent
state model is not linked into this execution path.

## Executed acceptance checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 247 tests passed |
| `cargo test -p larp-compiler --release --locked --offline` | 98 tests passed |
| `python3 tools/check_corpus.py` | 69 definitions: 67 active, 2 pending |
| `python3 tools/run_conformance.py` | 67/67 executed cases passed |
| `python3 tools/run_conformance.py --gc-stress` | 67/67 executed cases passed |
| `sh tools/git-local.sh diff --check` | Passed |

Test counts refer to test functions, not assertions or normal/stress repetitions.
The remaining pending fixtures concern lazy bindings in M6. Passing this local
gate is not a complete v0.1 release or evidence of a hosted CI run.

## Compiler and region evidence

The new scenario compiler suite contains 23 tests. Positive programs run with
normal collection and with collection at every allocation safepoint, comparing
exact stdout, exact inspection output, and full managed-heap reclamation. The
original scenario fixtures now execute through the CLI, including a method call
on the view and an error result that remains ordinary data before selected commit.

Coverage includes conditional and repeated selection, selection before writes,
selection of final values after later writes, unselected discard, last claims
surviving ordinary writes, sequential entry baselines, ordinary alias visibility
after completion, fixed/option/result/ConversionError fields, snapshot imports,
inner shadowing, loops wholly inside the block, and fatal diagnostics with exact
source spans. `palantir` has a separate inspection sink, preserves declaration
order, and omits inaccessible fields.

Regional methods and helpers access performed state through explicit VM views.
Tests exercise extension calls, private method chains, a view in a non-leading
helper parameter, recursive and mutually recursive helpers, and eligible local
value callbacks with snapshot captures. Specialization memoizes graph targets;
a 600-frame recursive run does not create hundreds of specialized functions.

Rejected programs cover outer aliases and function bindings, captured outer
references, outer stores, copied views, root-bound method extraction, identity
comparison, role/reference-bearing roots, crossing return/break, nested blocks,
transitively nested scenarios, role dispatch hidden behind a helper, reference
allocation hidden behind calls, captured reference state, and declared I/O.
A 4096-import signature produces a resource diagnostic rather than malformed
bytecode. Observer references produce a source diagnostic rather than a panic.

The compiler encodes region qualification in function contexts: all snapshot
locals are `S_entry(T)` and the sole class denotes `View_entry(C)`. Calls instantiate
the same region. Ordinary ingress is limited to the dedicated entry instruction;
its result is unit. Eligibility depends on the complete specialized call/closure
graph, not solely a function effect flag. Boundaries persist through lowering.

## Independent verifier and runtime evidence

Eleven new adversarial bytecode tests construct modules independently of the
frontend. They reject invalid entry/region/origin metadata, forged source
ownership/signatures, ordinary/regional or cross-region calls and closures,
entry reentry, unsafe indirect signatures, reference captures/returns/locals,
foreign root operations, reference allocation, role dispatch, inaccessible or
out-of-order report fields, and missing effects even in unreachable code.
Origin checks preserve public/private method access instead of treating every
specialized helper as public. Two type tests cover recursive SnapshotValue
classification and bounded type work; an IR test checks metadata/operation
preservation through lowering and verification.

Eight new VM tests use the actual executor and a retained private heap. After
injected crash, assertion failure, integer overflow, fuel exhaustion, and
inspection-sink failure, they inspect the real original fields and confirm the
baseline survived despite earlier selection. Already emitted observations remain.
Other cases check final selected publication, claims surviving ordinary writes,
helper returns without early commit, and view forwarding across call frames.
Stress collection keeps a root alive even after ordinary root locals have been
overwritten: the active scenario state itself owns an explicit GC root.

A CLI test checks that `check` produces no output and `run` separates normal
stdout from inspection stderr in both GC modes. The conformance harness compares
explicit inspection expectations and permits declared observations before a
runtime error; it still rejects unexpected stderr for ordinary success cases.

## Practical evaluation

Four executable examples compare configuration tuning and game training against
explicit object copies. All four run under GC stress and are active corpus cases.
The release comparison compiles once, warms up, and records seven samples for
10,000 repeated trials per variant. Its six variants produce 42 raw samples,
check paired final results and full managed reclamation, and separate inspection
formatting from the primary state-management comparison.

See [scenario evaluation](scenario-evaluation.md), the
[benchmark source](../crates/larp-compiler/examples/scenario_comparison.rs), and
[raw CSV](data/scenario-comparison.csv) for measurements, methodology, and limits.
The local toy measurements do not establish general demand, total memory use,
or a cross-machine performance advantage. Snapshot vectors and VM frames use
host allocations outside managed-arena counters.

The compiled account counterexample ends with balance=10 and reserved=20 after
selecting only balance. The implementation preserves field types, not arbitrary
relationships between fields. The scenario feature remains experimental; this
local acceptance does not authorize expanding roots to object graphs or promising
transactional application invariants.

## Remaining scope

M6 still includes lazy bindings, `sybau`, persisted versioned bytecode and bounded
loader, formatting, minimal declared-effect host APIs, and developer-preview
release checks. Modules, nested/first-class scenarios, role views, arbitrary
object-graph snapshots, grouped invariants, and generalized effects require later
specifications. No concurrency, durable commit, rollback of output, sustained
fuzzing result, host OOM guarantee, or wall-clock bound for an individual built-in
is claimed. There is no remote repository or distribution license selected.
