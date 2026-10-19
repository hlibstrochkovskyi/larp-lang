# v0.1 developer-preview readiness audit

Date: 2026-10-19. **The preview gate remains open.** The project has an
executable compiler/VM and candidate archives; neither a green finite corpus nor
this audit approves the whole language specification. This document connects
requirements to inspectable evidence and records counterexamples that still need
resolution. It does not replace the [specification](spec/v0.1.md),
[implementation status](implementation-status.md), or [roadmap](roadmap.md).

"Exercised" below means working implementation and relevant executed tests exist.
It does not mean every possible program or every normative sentence is proved.
"Open" means a counterexample, missing decision, or insufficient release evidence
still prevents closing that requirement.

## Language and execution evidence

| Requirement | Evidence | Finding |
| --- | --- | --- |
| §1: independent compilation and sequential execution | [Compiler entry](../crates/larp-compiler/src/lib.rs), [typed IR](../crates/larp-ir/src/lib.rs), [VM](../crates/larp-vm/src/lib.rs), CLI disassembly tests | Exercised; source compiles to verified bytecode, without host eval or translation |
| §1: left-to-right evaluation and checked assignment places | [Core tests](../crates/larp-compiler/tests/core.rs), [collection tests](../crates/larp-compiler/tests/collections.rs) | Exercised, including bounds-before-RHS and nested fixed-array sibling writes |
| §2: lexical forms, comments, escapes, UTF-8 locations | [Frontend tests](../crates/larp-syntax/tests/frontend.rs), [source tests](../crates/larp-source/src/lib.rs) | Exercised; Unicode strings and byte/scalar distinctions are explicit |
| §2: exact grammar acceptance | [Grammar](spec/grammar.ebnf), parser `is_place`/grouping | **Open G1:** grouped assignment places are accepted beyond the published production |
| §3: exact numeric types, contextual literals, conversions, IEEE behavior | Core/function tests; [context regressions](validation-contextual-equality.md) | Exercised; recent operand-order defects show why passing earlier fixtures was insufficient |
| §3: equality and invariant payloads | Function, collection, object, and role tests | Exercised for supported payloads/identities; dynamic-array/function equality is rejected |
| §4: scopes, parameters, captures, forward calls and recursion | [Function tests](../crates/larp-compiler/tests/functions_and_results.rs), core tests | Exercised; captures are snapshots and ordinary prelude names can be shadowed |
| §4: return paths and constant control flow | Compiler `finish` and IR `reachable` | **Open G2:** impossible constant-condition successors still count as reachable |
| §4: loop bounds, read-only indices, breaks and short circuit | Core tests and conformance loop/short-circuit cases | Exercised, including end-exclusive integer ranges and skipped constant evaluation |
| §4: local lazy cells, captures, forcing and abort effects | [Lazy tests](../crates/larp-compiler/tests/lazy_and_suppression.rs), [lazy VM tests](../crates/larp-vm/src/lazy_tests.rs) | Exercised; SnapshotValue captures/results and memoized forcing are checked |
| §5: nominal classes, visibility and constructor publication | [Object tests](../crates/larp-compiler/tests/objects.rs), [object validation](validation-objects.md) | Exercised; no partially initialized identity is published |
| §5: roles, bound methods, exact extensions and member priority | [Role tests](../crates/larp-compiler/tests/roles.rs), [bound built-ins](../crates/larp-compiler/tests/bound_builtins.rs) | Exercised in source and independently verified bytecode |
| §6: dynamic/fixed arrays and callback iteration guards | Collection tests, [collection validation](validation-collections.md) | Exercised with normal/stress GC and saved receiver/callback state |
| §6: option/result, eager recovery and conversion errors | Function tests and active conversion/recovery cases | Exercised; expected errors remain values, fatal failures terminate |
| §7: effect bounds through indirect and role/callback edges | Compiler tests and [bytecode tests](../crates/larp-bytecode/tests) | Exercised; callback/alias routes retain checks |
| §7: suppression and independent observation sinks | Lazy/suppression tests and scenario cases | Exercised; arguments still execute, inspection and failures remain observable |
| §7.1: explicit input effects and independent host capabilities | [Input tests](../crates/larp-compiler/tests/host_input.rs), [host contract](host-input.md) | Exercised; EOF, invalid UTF-8, I/O failure and denied/limited capabilities remain distinct |
| §8.1–8.3: snapshot eligibility, regional calls and escape rejection | [Scenario compiler tests](../crates/larp-compiler/tests/scenarios.rs), [scenario verifier tests](../crates/larp-bytecode/tests/scenarios.rs) | Exercised; nested regions/reference graphs remain outside v0.1 |
| §8.4: final-field commit, claims, abort discard and alias visibility | [Scenario VM tests](../crates/larp-vm/src/scenario_tests.rs) | Exercised; tests inspect retained real heap after injected failures, rather than infer preservation from process output |
| §8.5: required program and practical alternatives | Active scenario/configuration/game cases; [evaluation](scenario-evaluation.md) | Executed; selective commit can break cross-field invariants and stays experimental |
| §9: traced lifetime, cycles and temporary roots | Heap/VM/compiler stress tests; [resource measurements](validation-resources.md) | Exercised; logical arena counters are not byte budgets or whole-process memory guarantees |
| §10: stable diagnostic data and exact runtime spans | [Protocol](diagnostics.md), renderer/CLI tests, all ten runtime contracts | Exercised in text/JSON, formatted source and loaded artifacts; this closes a previously weaker location-presence check |
| §11: baseline design review | Numeric conversion, finite effects, conservative regions, selective-commit limitation | **Open:** implementation does not constitute maintainer acceptance of these policies |

## Confirmed reconciliation work

### G1: grouped assignment places

The following source currently executes and prints `2\n`:

```larp
main_character { mf x = 1; (x) = 2; yap(x); }
```

The grammar's `place` production starts with IDENT or `self`, without grouped
roots. The parser discards grouping before checking `is_place`, so its accepted
syntax is wider. Reconcile the production with an explicit grouped-place policy,
or preserve enough grouping information to reject it. Do not describe the
current grammar as exact until that decision is reflected in grammar and tests.

### G2: constant-condition return reachability

This source currently fails with `LARP-T007`:

```larp
sidequest answer() -> mf uses() {
    if (fr) { return 42; }
}
main_character { yap(answer()); }
```

Section 4 requires a return on every reachable non-aborting path. The compiler
emits both successors for a constant condition, and structural IR reachability
visits both. Its return check consequently treats the impossible fallthrough as
live. Reconcile constant `if`/`while` control flow and constructor initialization
without skipping type/effect checks in dead branches. Add positive and negative
execution tests before claiming this rule is satisfied.
Under the current normative wording, the example should compile and print
`42\n`; conservatively rejecting it is not a completed implementation of that rule.

## Toolchain and distribution gates

| Gate | Evidence and remaining work |
| --- | --- |
| Real conformance execution | 85 active cases, no pending entries; normal/stress, loaded artifacts, and formatted loaded artifacts execute in CI. The inventory is finite, not complete semantic coverage. |
| Artifact loading and source maps | [Wire format](bytecode-format-v1.md), [v2 input additions](bytecode-format-v2.md), loader/verifier/CLI tests; VM execution accepts verified modules only. |
| Source formatter | [Formatter contract](formatter.md), lossless syntax tests, and formatted conformance execution; no token/spelling/order rewrite. |
| Robustness and resource evidence | [Bounded campaigns](validation-fuzzing.md) and fresh-process measurements exist. No sustained coverage, hostile-code sandbox, heap-byte limit, or universal resource-bound proof is claimed. |
| Reference programs | [Reference contracts](reference-cases.json) execute all eight complete reference examples through real source/artifact/GC paths. |
| License and attribution | Maintainer-selected [MIT](../LICENSE); candidate packages retain notices from their actual Rust distribution. |
| Reproducibility and extraction | Two independent native builds and archives are compared byte-for-byte; extracted binaries run real programs. See [build](reproducible-builds.md) and [packaging](distribution.md) contracts. |
| Hosted exact-commit checks | Compiler/conformance/packaging and bounded fuzz jobs passed for [`a84f033`](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39178705380) and [its campaign](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39178705339). Later commits need their own outcomes. |
| Platform policy | **Open:** current candidates are native GNU/Linux x86-64. Build-host/runner success does not establish a minimum GLIBC/distribution/kernel or other platforms. Choose and publish a concrete preview support policy. |
| Versioned distribution | **Open:** the candidate workflow uploads reviewable archives; it creates neither a version tag nor a GitHub Release. Define preview version/compatibility/update policy before publication. |

Additional host APIs, modules/packages, user generics, nested/object-graph
scenarios, async, native optimization and editor services remain later RFC work.
They are not implemented by renaming existing syntax or by closing a finite
fixture inventory. Keep the broader project roadmap explicit when defining a
bounded first preview.
