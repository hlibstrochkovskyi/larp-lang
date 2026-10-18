# Classes, constructors, and object lifetime validation

Historical class-slice snapshot. See [roles and extensions validation](validation-roles-and-extensions.md)
for the following M4 acceptance gate and [implementation status](implementation-status.md)
for current coverage.

Date: 2026-10-17. Environment: Linux, rustc/Cargo 1.99.0. The dependency-free
workspace builds offline. This report records local validation of the class
slice within M4. Roles and extensions remain pending; M4 and Specification v0.1
are not complete.

## Executed checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 154 tests passed |
| `cargo test -p larp-compiler --release --locked --offline` | 55 integration tests passed |
| `python3 tools/check_corpus.py` | 57 valid fixture definitions: 43 active, 14 pending |
| `python3 tools/run_conformance.py` | 43/43 active cases passed |
| `python3 tools/run_conformance.py --gc-stress` | 43/43 active cases passed |
| `sh tools/git-local.sh diff --check` | Passed |

The workspace total extends the preceding 118 tests with seven class syntax
tests, ten class-focused bytecode-verifier tests, 14 compiler object integration
tests, three VM object tests, and two heap tests. The compiler release suite
contains 14 scalar/core, 13 function/recovery, 14 collection, and 14 object tests.
Each test function can exercise multiple programs or rejection cases; individual
assertions and normal/stress repetitions are not counted as separate tests.

The class example was executed with:

```sh
cargo run -p larp-cli --locked --offline -- run examples/objects.larp --gc-stress
```

Its stdout is `unc\n20\nfr\n`. The saved `bro.train` method mutates the same
object later inspected through public fields and `bro.isMogging(2)`. Source
programs execute through semantic checking, typed control-flow IR, bytecode
lowering, mandatory verification, and the VM.

## Class and constructor evidence

Compiler integration tests exercise nominal class identity, shared mutation
through aliases, distinct identities for separate constructions, public/private
access, direct calls, extracted bound methods, and nested lexical closures.
Private methods and fields cannot be accessed from outside their declaring class,
including through aliases or an outside lambda. A class method may intentionally
return a closure or bound method with private access; invocation retains its
declared effects. Bound methods save their original receiver, and receiver
evaluation precedes argument evaluation.

Constructor cases cover initialized-field reads, branch joins, early `return;`,
loop exits, and successful fallthrough. Missing fields and reads before definite
assignment produce `LARP-T011` with source locations rather than an internal
bytecode failure. Constructors can create fresh nested objects and store a
deferred callback that later mutates an ordinary captured array. Calling that
write-bound callback during construction is rejected.

Escape cases reject bare/captured `self`, nested capture of `self`, method calls
or bound-method extraction on partial `self`, and explicit constructor return
values. External object writes, array writes/growth, and write-bound direct or
indirect calls are rejected during initialization. Observation exceeds the
constructor's implicit `read,write,abort` bound. An aborting constructor reports
`LARP-R003` at the `crashout` expression and emits no following observation.

Additional cases reject receiver reassignment, nominally incompatible classes,
unknown class names, incorrect constructor arity, duplicate members/types,
missing or duplicate constructors, and invalid field types. Object fields compose
with fixed-array places: writeback preserves a sibling update performed by the
right side, and compound assignment reads its previous value first. Field/method
results preserve numeric literal context. A method named `isSome` returning
`unc` is resolved from its declared class signature rather than a prelude method
name heuristic.

## Bytecode, runtime, and GC evidence

Adversarial bytecode tests validate nested nominal identities/names, member
layouts, owner/receiver metadata, constructor registration/effects, and lexical
private access. Ordinary calls and closure creation cannot target constructors;
only `Construct` enters them. Only the registered constructor may use terminal
`NewObject`, which requires exactly the ordered field values and returns the
fresh object directly. Crafted constructors cannot substitute an existing class
parameter via ordinary `Return`, leave extra operands, or continue after
publishing an identity. Receiver stores, forged field receivers/payloads, external
writes, and observation are rejected, including forbidden unreachable operations.
Exported private capabilities remain callable indirectly with their effects.

Independent VM tests exercise alias identity, private closures, object fields
containing objects/arrays/callbacks, constructor temporaries across nested
allocations, and abort before publication. Successful compiler/VM programs run
under normal collection and collection at every allocation safepoint, comparing
output and asserting that every allocated heap entry is reclaimed after exit.

A source program creates 500 two-object cycles while retaining a bound method
whose receiver's creating frame has exited. Stress collection preserves its
output `49\n`, keeps fewer than 20 heap entries live at peak, and reclaims all
allocated entries by termination. Heap tests additionally trace a live cycle
across object, array, closure, and tagged/fixed wrappers, then reclaim it once
unrooted. Generation reuse preserves identity distinctions and prevents stale
handles from resolving. A 20,001-object chain is marked and reclaimed without
recursive graph destruction.

## Limits of the evidence

The class slice does not complete M4: roles, class-to-role coercion, interface
dispatch, extensions, and member/extension priority remain pending. Extracting
bound methods is implemented for class methods; built-in collection method
extraction is still pending. Scenarios, lazy bindings, `sybau`, persisted bytecode,
modules, host I/O APIs, and formatting remain unimplemented. The 14 pending
fixtures are not passing language programs.

Constructor effect checking is conservative: ordinary helpers or callbacks with
a `write` bound are forbidden even when a particular helper would only initialize
fresh storage. Dedicated nested `new` remains allowed. Creating a deferred
write-bound callback does not execute it, but capturing partial `self` is always
rejected. Definite initialization does not prove arbitrary conditions or loop
invariants.

Stress and adversarial tests provide evidence, not a full soundness proof,
sustained fuzzing result, or performance guarantee. Tests do not prove host OOM
behavior or bound wall-clock cost of individual built-ins. Lazy/scenario roots
need future GC integration. The separate scenario model does not establish
compiled region or commit safety. CI configuration is local repository state;
no hosted CI run, remote repository, or distribution license is claimed.
