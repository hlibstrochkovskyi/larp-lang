# Preview baseline technical review

Date: 2026-10-19. Status: **technical review completed; maintainer design
acceptance remains open**. This review evaluates the four choices called out by
[specification §11](spec/v0.1.md#11-deferred-changes-and-review-gates). It records
implementation evidence and recommendations, not approval on the maintainer's
behalf or permission to publish a release.

## Numeric conversions

Recommendation: retain exact numeric types and explicit integer conversions for
the first preview. `mf` and `unc` do not implicitly widen or narrow; identity and
`mf`/`unc` conversions return checked results through `ascend`, while `fraud`
aborts on failure. Floating-point conversions remain deferred. The target type
determines the result; it does not automatically give an otherwise unconstrained
argument that same integer type. For example, `ascend<unc>(3_000_000_000)` still
has an out-of-range default `mf` argument unless its context supplies `unc`.

Evidence: [conversion signatures](../crates/larp-types/src/intrinsic.rs),
[call checking](../crates/larp-compiler/src/semantics/calls.rs), and
[function/result tests](../crates/larp-compiler/tests/functions_and_results.rs).
The review exercised identity/widen/narrow success, rejected implicit widening
and floating conversions, and observed `LARP-R008` for failed `fraud`.

## Finite effect bounds

Recommendation: retain the explicit finite upper bounds `read`, `write`,
`observe`, `io`, and `abort`. An omitted declaration permits `abort`; `uses()`
permits none of these semantic effects. Bounds remain part of function and
callback types; widening a function's allowed effects does not make its later
invocation legal in a narrower caller. These are conservative declared upper
bounds, not a claim of inferred minimal effects or an algebraic-handler system.

Evidence: [effect types](../crates/larp-types/src/lib.rs),
[semantic declarations](../crates/larp-compiler/src/semantics.rs),
[core tests](../crates/larp-compiler/tests/core.rs), and function/result tests.
The review confirmed the default abort bound, rejection of undeclared observation,
and preservation of a widened function bound at invocation.

## Conservative scenario regions

Recommendation: retain one concrete SnapshotValue-field root and conservative
transitive regional checks. Imported values are immutable snapshots; ordinary
inner shadowing is allowed. Outer aliases and reference-bearing allocations in
reachable helpers are rejected. Eligibility scans all typed blocks, including
reference allocation inside a constant-false branch. Feasible-path return
analysis does not erase this separate regional restriction.

Evidence: [regional specialization](../crates/larp-compiler/src/semantics/scenarios.rs),
[independent bytecode checks](../crates/larp-bytecode/src/scenarios.rs), and
[scenario compiler tests](../crates/larp-compiler/tests/scenarios.rs).
The review executed immutable-entry/shadowing behavior and rejected an outer alias
and transitive dead-branch reference allocation. The restriction trades expressivity
for a checkable initial model; it does not promise information-flow security.

## Selective commit and application invariants

Recommendation: keep scenarios experimental and state their limitation beside
the first example. `canon` selects a field's final value, including writes after
selection. Publication replaces a prepared complete field vector without user
callbacks, so aliases of the original identity observe the completed result.
Fatal failure discards the trial; an ordinary `fumble` value does not cancel it.

Selective field commit preserves declared types, not relationships between
fields. The review produced real state `a = 10, b = 20` after a trial whose full
performed state satisfied `a >= b`; committing only `b` made that invariant false.
This is an application-level limitation, not a transaction rollback defect.
Do not describe this feature as financial safety or general transactional I/O.

Evidence: [publication implementation](../crates/larp-vm/src/scenario.rs),
[heap-inspecting rollback tests](../crates/larp-vm/src/scenario_tests.rs),
scenario compiler tests, and the executed [practical comparison](scenario-evaluation.md)
against explicit-copy configuration/game simulations.

## Executed scope

An independent review ran twelve fixed positive/negative CLI cases with GC stress
against the existing `0.1.0-dev` binary, then replayed exactly those cases against
the rebuilt `0.1.0-preview.1` binary. All twelve met their exact expected status,
stdout, and primary diagnostic contracts in both runs. The preview CLI SHA-256 was
`031ae666a581228e78c63109913975f33b0dfd4340e2f68adf466fc401ec6c6f`.

Retained ignored evidence includes source programs, stdout/stderr, result records,
and binary identity in `artifacts/preview-semantic-review/` and
`artifacts/preview-semantic-review-preview1/`. These local debug-binary executions
do not identify the separately packaged release binary or establish Ubuntu
compatibility. The [preview notes](releases/0.1.0-preview.1.md),
[readiness audit](preview-readiness.md), and exact-commit hosted gates retain
those separate responsibilities. Finite tests and this review do not prove every
program correct or settle the maintainer's design preferences.
