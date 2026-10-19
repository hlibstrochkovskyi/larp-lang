# v0.1 developer-preview readiness audit

Audit baseline: 2026-10-19. **The bounded first preview is now published as
[`v0.1.0-preview.1`](releases/0.1.0-preview.1-publication.md).** Its publication
record identifies the approved source commit, archived binary, and passing
release checks. Neither publication nor a green finite corpus approves the
whole proposed specification. This document connects requirements to inspectable
evidence and records identified counterexamples and their resolution. It does
not replace the [specification](spec/v0.1.md),
[implementation status](implementation-status.md), or [roadmap](roadmap.md).

"Exercised" below means working implementation and relevant executed tests exist.
It does not mean every possible program or every normative sentence is proved.
"Open" identifies a remaining design decision or an unsupported broader claim;
it does not imply that the explicitly bounded published preview is unreleased.

## Language and execution evidence

| Requirement | Evidence | Finding |
| --- | --- | --- |
| §1: independent compilation and sequential execution | [Compiler entry](../crates/larp-compiler/src/lib.rs), [typed IR](../crates/larp-ir/src/lib.rs), [VM](../crates/larp-vm/src/lib.rs), CLI disassembly tests | Exercised; source compiles to verified bytecode, without host eval or translation |
| §1: left-to-right evaluation and checked assignment places | [Core tests](../crates/larp-compiler/tests/core.rs), [collection tests](../crates/larp-compiler/tests/collections.rs) | Exercised, including bounds-before-RHS and nested fixed-array sibling writes |
| §2: lexical forms, comments, escapes, UTF-8 locations | [Frontend tests](../crates/larp-syntax/tests/frontend.rs), [source tests](../crates/larp-source/src/lib.rs) | Exercised; Unicode strings and byte/scalar distinctions are explicit |
| §2: exact grammar acceptance | [Grammar](spec/grammar.ebnf), [grouped-place tests](../crates/larp-compiler/tests/grouped_places.rs) | G1 resolved: the production explicitly permits grouping; temporary roots and immutable targets remain rejected |
| §3: exact numeric types, contextual literals, conversions, IEEE behavior | Core/function tests; [context regressions](validation-contextual-equality.md) | Exercised; recent operand-order defects show why passing earlier fixtures was insufficient |
| §3: equality and invariant payloads | Function, collection, object, and role tests | Exercised for supported payloads/identities; dynamic-array/function equality is rejected |
| §4: scopes, parameters, captures, forward calls and recursion | [Function tests](../crates/larp-compiler/tests/functions_and_results.rs), core tests | Exercised; captures are snapshots and ordinary prelude names can be shadowed |
| §4: return paths and constant control flow | Compiler `branch`/`finish`, IR reachability, [constant-flow tests](../crates/larp-compiler/tests/constant_flow.rs) | G2 resolved for established scalar conditions and range entry; dead code still receives semantic checks and settled inferred returns are validated |
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
| §11: baseline design review | [Technical review](preview-design-review.md) of numeric conversion, finite effects, conservative regions and selective-commit limitations | Technical review completed with executable cases. The maintainer approved publication of the bounded preview; broader specification/design acceptance remains open and is not implied by that decision. |

## Resolved reconciliation work

### G1: grouped assignment places — resolved

The following source currently executes and prints `2\n`:

```larp
main_character { mf x = 1; (x) = 2; yap(x); }
```

The earlier production started with IDENT or `self`, while the parser already
accepted grouping. The production now recursively permits parenthesized places.
Syntax and compiler tests exercise grouped local/self/member/index paths,
read-only targets, temporary rejection, once-only index evaluation,
bounds-before-RHS behavior and sibling-preserving fixed-array writes.

### G2: constant-condition return reachability — resolved

This source previously failed with `LARP-T007`; it now executes with stdout `42\n`:

```larp
sidequest answer() -> mf uses() {
    if (fr) { return 42; }
}
main_character { yap(answer()); }
```

Known scalar conditions now choose a single CFG successor. This applies to
`if`, `while` and short-circuit evaluation. Range lowering separately checks the
saved initial bounds, preserving proof of zero/at least one iteration without
treating later index tests as constant. Return checks and constructor
initialization use these feasible paths. Unknown conditions remain conservative.

Both source branches are still checked, including effects and private/region
access. Regional specialization scans all typed blocks before bytecode lowering.
Inferred lambda results receive a final compatibility check for earlier ordinary
dead returns, while nonreturning expressions do not supply false `unit` results.
No nested-body replay or whole-body type unification is introduced.
See [control-flow validation](validation-control-flow.md) for positive/negative
evidence and the explicit limits of sequential literal context.

## Toolchain and distribution gates

| Gate | Evidence and remaining work |
| --- | --- |
| Real conformance execution | 89 active cases, no pending entries; normal/stress, loaded artifacts, and formatted loaded artifacts execute through the harness. The inventory is finite, not complete semantic coverage; current-commit CI must be checked separately. |
| Artifact loading and source maps | [Wire format](bytecode-format-v1.md), [v2 input additions](bytecode-format-v2.md), loader/verifier/CLI tests; VM execution accepts verified modules only. |
| Source formatter | [Formatter contract](formatter.md), lossless syntax tests, and formatted conformance execution; no token/spelling/order rewrite. |
| Robustness and resource evidence | [Bounded campaigns](validation-fuzzing.md) and fresh-process measurements exist. No sustained coverage, hostile-code sandbox, heap-byte limit, or universal resource-bound proof is claimed. |
| Reference programs | [Reference contracts](reference-cases.json) execute all eight complete reference examples through real source/artifact/GC paths. |
| License and attribution | Maintainer-selected [MIT](../LICENSE); candidate packages retain notices from their actual Rust distribution. |
| Reproducibility and extraction | Two independent native builds and archives are compared byte-for-byte; extracted binaries run real programs. See [build](reproducible-builds.md) and [packaging](distribution.md) contracts. |
| Hosted exact-commit checks | The published source commit `61cc9314916c510673112dc799e247c0f59cc3b9` passed all five [candidate CI jobs](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39248451254). Earlier [compiler](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39178705380) and [bounded-fuzz](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39178705339) outcomes remain historical evidence, not proof for arbitrary later commits. See the [publication record](releases/0.1.0-preview.1-publication.md) for the release checks. |
| Platform policy | [Explicit policy](platforms.md): Ubuntu 24.04 build and identical-archive execution on Ubuntu 24.04/26.04, x86-64 GNU/Linux. Both host jobs and their digest/conformance aggregate passed for the published `61cc931` archive; see the [publication record](releases/0.1.0-preview.1-publication.md). Later candidates require their own evidence. |
| Versioned distribution | [`v0.1.0-preview.1`](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.1.0-preview.1) is published after explicit maintainer approval. A machine-checked descriptor binds the frozen [release notes](releases/0.1.0-preview.1.md) and formats to the actual CLI/archive. The separate [publication record](releases/0.1.0-preview.1-publication.md) retains the exact commit, tag, artifacts, and post-upload checks. |

Additional host APIs, modules/packages, user generics, nested/object-graph
scenarios, async, native optimization and editor services remain later RFC work.
They are not implemented by renaming existing syntax or by closing a finite
fixture inventory. Keep the broader project roadmap explicit when defining a
bounded first preview.
