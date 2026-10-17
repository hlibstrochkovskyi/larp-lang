# Implementation roadmap

Status: foundation established; compiler milestones are pending. This is a
sequence of acceptance gates, not a delivery-date promise.

## M0 — Reviewable design and repository foundation (current)

- Independent English design review, proposed specification, grammar, and sources.
- Rust workspace, source locations, diagnostic registry, honest CLI shell, CI.
- Untracked private notes; Git main branch with the maintainer's identity.
- Isolated executable state model and a pending conformance inventory.

Gate: local formatting, Clippy, workspace tests, and corpus inventory checks pass.
The proposed design still needs maintainer review; model tests do not approve it.

## M1 — First real compiler-to-VM slice

Implement only scalar locals, arithmetic, comparisons, `if`/`else`, and `yap` in
`main_character`. Include the lexer, parser, resolver/type checker, minimal typed
IR, in-memory bytecode, verifier, and VM. Do not add OOP syntax to the parser
before the core path is runnable.

Gate: `larp check` rejects truthiness and implicit conversion; `larp run` executes
the scalar fixture through bytecode; `larp explain` maps emitted errors to the
same registry. Add negative, malformed-input, Unicode-span, and checked-overflow
tests. A disassembly test proves the path really emits bytecode.

First implementation issue: **Compile scalar main blocks to verified bytecode**.
Acceptance scope: `mf`, `unc`, `verdict`, `text`, literal/local expressions,
`+ - *`, comparisons, `if`/`else`, `no_cap`, `yap`, and structured errors. CLI must explicitly
diagnose syntax outside this subset. This issue can start after design review;
its implementation is not part of the setup.

## M2 — Usable algorithmic core

Functions, recursion, lexical scopes, branches, all loop forms, return checks,
exact numeric semantics, option/result, checked conversions, and fatal errors.

Gate: execute loop/factorial/error fixtures; test recursion bounds, division
edge cases, short-circuiting, invalid return paths, and multiple diagnostics
without cascades. `parseScore` and recovery behave as specified.

## M3 — Collections and functions as values

Built-in generic types, arrays, closures, eager collection operations, dot calls,
local inference, explicit function types, and effect-bound preservation.

Gate: the collection pipeline produces `5\n50\n`; callback mutation through an
alias aborts; captured local bindings are snapshots; higher-order calls cannot
erase effects. No user-defined generic declarations required yet.

## M4 — Objects and managed lifetime

Nominal classes, constructor checking, visibility, methods, roles, extension
resolution, heap identities, and a collector capable of collecting cycles.
Introduce GC earlier if collections/closures already need it; M4 completes its
object coverage rather than postponing all collection to here.

Gate: public/private cases, role dispatch, exact member priority, identity
aliasing, constructor escape rejection, and collection under stress mode pass.
Tests distinguish live cycles from unreachable cycles and exercise temporary
roots during nested allocations.

## M5 — Controlled alternative state

Review model findings first. Implement region qualification, transitive scenario
eligibility, view receivers, snapshot frames, claim history, selected final-field
commit, and observation. Keep roots value-only and scenarios non-nested.

Gate: scenario fixtures run through the compiler and VM; outer alias access,
outer stores, indirect unsafe calls, private access, reference roots, nested
blocks, and crossing exits are rejected. Inject fatal errors before commit and
check baseline preservation in VM tests. Assert all selected fields become
visible together and last claims remain distinguishable from ordinary writes.
Bytecode verification must reject crafted violations as well.

Compare explicit-copy and scenario versions of configuration tuning and game
statistics. Record code size, explanation burden, and failure modes. Decide
whether the feature earns its complexity before expanding it. Do not claim
financial safety or invariant preservation from field-level type safety.

## M6 — Developer preview

Persisted versioned bytecode with a bounded loader/verifier; useful source maps,
CLI integration tests, formatter, examples, reference docs, reproducible builds,
and minimal host APIs with declared effects. Introduce lazy bindings only once
effect and capture restrictions are enforced.

Gate: all v0.1 corpus cases are executed by a real harness; no pending cases in
the declared release subset. Fuzz lexer/parser/loader and measure VM/GC resource
limits. Choose a distribution license and supported platforms. Run CI on the
remote when a remote exists; local checks do not prove hosted CI passed.

## Later RFCs

Modules/packages, user-defined generics, richer pattern matching and result
propagation, scenario validation groups, nested/first-class scenarios, object
graphs, generalized effects, async, native backend, optimizer, debugger, LSP.
Keep the typed IR backend-neutral, but do not promise native ABI compatibility.
