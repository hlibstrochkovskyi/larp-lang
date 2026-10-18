# Roles, extensions, and bound built-ins validation

Date: 2026-10-18. Environment: Linux, rustc/Cargo 1.99.0. The dependency-free
workspace builds offline. Role dispatch, concrete extensions, and built-in
method extraction complete the M4 feature set and its local acceptance gate.
Workspace, release, repository, and both conformance modes passed locally. This
is not a full Specification v0.1 release.

## Acceptance checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | 194 tests passed |
| `cargo test -p larp-compiler --release --locked --offline` | 75 integration tests passed |
| `python3 tools/check_corpus.py` | 64 valid fixture definitions: 51 active, 13 pending |
| `python3 tools/run_conformance.py` | 51/51 active cases passed |
| `python3 tools/run_conformance.py --gc-stress` | 51/51 active cases passed |
| `sh tools/git-local.sh diff --check` | Passed |

The workspace total extends the preceding 154 tests with five syntax tests,
ten bytecode-verifier tests, five VM tests, 15 compiler role/extension tests, and
five bound-built-in tests. The compiler suite contains 75 integration tests.
Tests describe multiple programs and rejection cases; normal/stress repetitions
and individual assertions do not count as separate test functions.

## Executed example

The roles example was executed through the real CLI with:

```sh
cargo run -p larp-cli --locked --offline -- run examples/roles.larp --gc-stress
```

Its stdout is `7\n49\n14\n98\n`. Two nominal classes implement `Scored`; a
`mfs<Scored>` stores both object identities and invokes their concrete `score`
methods. The concrete `Scored.doubled` extension calls the role method, and an
eager collection pipeline observes the results. The compiler checks the source,
lowerer emits bytecode, mandatory verification runs, and the VM executes it.
The independent suite covers the additional acceptance cases below.

## Acceptance coverage

The passing compiler tests cover complete public role implementations with exact
parameter/result signatures and bounded effects; contextual conversion at
bindings, arguments, returns, fields, and literal payloads; concrete dispatch;
and identity preserved through role aliases. Existing containers and function
signatures remain invariant rather than being recursively converted. Bound
role methods retain their original receiver while observing later mutation of
that object. Declaration errors, incompatible implementations, and attempts to
instantiate a role must produce frontend diagnostics with source locations.

Extension cases cover exact static receiver types, extracted bound values,
receiver immutability, public state mutation through reference receivers, and
member priority. A real member with an incompatible signature or inaccessible
visibility must not fall back to an extension. Extensions do not acquire private
access. Direct calls, role calls, bound methods, and callback invocation must
retain declared effect bounds, including conservative constructor restrictions.

Bound built-in tests cover inferable monomorphic methods and contextually typed
higher-order methods. Receiver evaluation occurs at extraction: mutable objects
and dynamic arrays retain identity, while fixed arrays and tagged values are
snapshots. Callback signatures and effects remain present in the wrapper type;
extracted iteration methods use the ordinary alias-mutation guards. Returning a
saved method, retaining a function payload through recovery, and cyclic receiver
graphs exercise roots across normal and stress collection.

The bytecode tests independently construct malformed in-memory modules. Coverage
includes incomplete/private/wrong-name implementation entries, mismatched method
signatures/effects, duplicate nominal declarations, forged nested role types,
invalid conversion/dispatch operands and indices, unreachable effect violations,
constructor writes through role calls, and invalid extension receiver/ownership
metadata. Shared type DAGs are bounded before structural comparisons. These checks run
even when the frontend would reject the source earlier. Disassembly exposes
role signatures, class implementation tables, and concrete extension metadata,
so the resolved dispatch contracts can be inspected before execution.

## Limits of the evidence

M5 remains unimplemented: scenario regions, transitive scenario eligibility,
`larp/flex/canon/palantir`, commit preparation, and discard behavior have not been
validated through compiled programs. Role and function effect bounds alone do
not establish scenario safety. The independent state model remains an experiment.

M6 remains pending: persisted versioned bytecode and loader, lazy bindings,
`sybau`, formatting, host I/O APIs, and developer-preview release gates. Modules
remain deferred.
There is no class or role inheritance, downcast, user-defined generic extension,
or cross-module extension lookup. Function parameters, ordinary local bindings,
and array elements cannot have type `unit` in the implemented subset.

Adversarial tests and stress collection provide evidence, not a soundness proof,
sustained fuzzing result, host OOM guarantee, or wall-clock bound for individual
built-ins. Lazy/scenario roots need additional collector integration. The 13 pending
fixtures are not passing language programs. No hosted CI run, remote repository,
or distribution license is claimed.
