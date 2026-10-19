# Implementation roadmap

Status: M1 through the M5 feature set are implemented. M6 is in progress; lazy
bindings, output suppression, persisted bytecode, and source formatting are
implemented, while preview gates remain open.
This is a sequence of acceptance gates, not a delivery-date promise.

## M0 — Reviewable design and repository foundation (established)

- Independent English design review, proposed specification, grammar, and sources.
- Rust workspace, source locations, diagnostic registry, honest CLI shell, CI.
- Untracked private notes; Git main branch with the maintainer's identity.
- Isolated executable state model and a pending conformance inventory.

Gate: local formatting, Clippy, workspace tests, and corpus inventory checks pass.
The proposed design still needs maintainer review; model tests do not approve it.

## M1 — First real compiler-to-VM slice (implemented)

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
it is now implemented with active CLI conformance tests.

## M2 — Usable algorithmic core (implemented feature set)

Functions, forward/mutual recursion, lexical scopes, branches, all loop forms,
return checks, exact scalar arithmetic, fatal errors, and finite effect bounds.
Options/results, recovery callbacks, checked integer conversions, and
`parseScore` now execute through typed IR, verified bytecode, and the VM. Recovery
uses the first-class functions and closure machinery introduced ahead of M3.

Gate: execute loop/factorial/error fixtures; test recursion bounds, division
edge cases, short-circuiting, invalid return paths, and multiple diagnostics
without cascades. `parseScore` and recovery behave as specified.

## M3 — Collections and functions as values (implemented feature set)

Implemented: first-class named/prelude functions, explicit function types,
contextual and annotated lambdas, immutable snapshot captures, nested/returned
closures, and effect-bound preservation through indirect calls. Built-in generic
option/result types and their recovery dot calls are available. Array/closure
memory is managed by a non-moving tracing collector with stress mode.

Dynamic/fixed arrays support contextual literals, checked indexing, indexed
assignment, and `size()`. Dynamic arrays provide `push`, stable `sorted`, eager
`filter`, `map`, and `forEach`. Built-in callbacks preserve their effect bounds;
iteration guards reject structural mutation through any receiver alias. The VM
uses explicit suspended iterations rather than recursive host callback calls,
and traces their receiver, callback, and unfinished outputs. Array/closure cycles
are collectible. User-defined generic declarations remain deferred.

Gate: the collection pipeline produces `5\n50\n`; structural callback mutation
through an alias aborts; captured local bindings are snapshots; higher-order
calls cannot erase effects. No user-defined generic declarations required yet.

## M4 — Objects and managed lifetime (implemented)

Implemented class slice: nominal `archetype` declarations, one checked `init`,
public/private fields and methods, immutable `self` bindings, direct instance
calls, extracted bound methods, and lexical private access in nested closures.
Constructor field locals undergo definite-initialization analysis over the CFG;
only terminal allocation publishes a completed identity. Existing heap mutation,
observation, and I/O are forbidden during construction; nested construction is
allowed. Object fields participate in tracing, including object/array/closure
cycles and temporary roots during nested allocations.

Roles, contextual class-to-role conversion, verified interface dispatch, bound
role methods, and exact concrete extensions are implemented. Real members take
priority over extensions, including access and signature failures. Built-in
method extraction now generates closures with captured receivers and preserved
callback bounds; polymorphic methods require an expected concrete signature.
Frontend, adversarial bytecode, and normal/stress runtime checks exercise these
features. See [M4 validation](validation-roles-and-extensions.md) for executed
results and the limits of this local acceptance gate.

Gate: public/private cases, role dispatch, exact member priority, identity
aliasing, constructor escape rejection, and collection under stress mode pass.
Tests distinguish live cycles from unreachable cycles and exercise temporary
roots during nested allocations.

## M5 — Controlled alternative state (implemented, experimental)

The compiler now outlines lexical region entries and specializes their checked
call graphs. Dedicated VM view/snapshot frames implement claims, final-field
selection, inspection, and single-publication commit. Outer aliases/stores,
unsafe transitive calls, reference roots, crossing exits, and nested entries are
rejected by the frontend and independently constrained in bytecode verification.
Runtime tests inspect the retained real heap after injected failure and stress
collection. The local acceptance suite and practical comparisons are complete;
see [scenario validation](validation-scenarios.md) and
[scenario evaluation](scenario-evaluation.md). The feature remains experimental,
with value-only roots and no application-invariant guarantee.

Original acceptance scope: review model findings first. Implement region qualification, transitive scenario
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

## M6 — Developer preview (in progress)

Implemented: read-only snapshot-capturing `nonchalant` bindings, memoized forcing
with preserved abort bounds, local-only lazy cells, checked regional thunks, and
GC roots across forcing. Capturing another lazy binding forces and copies its
value immediately. `sybau` dynamically suppresses ordinary `yap` across calls
without removing observation effects or muting inspection/diagnostics. Source
return/break lowering and verified control-flow joins preserve nesting.

All 89 current corpus cases are active and executable. This closes the current
fixture inventory, not the developer-preview release gate or every normative
rule. See [lazy and suppression validation](validation-lazy-and-suppression.md).

Implemented artifact slice: deterministic versioned `.larpbc`, embedded source
maps, bounded decoding and mandatory verification, `build` and source/artifact
`check`/`run`/`dump`, deleted-source diagnostics, and corpus execution through loaded
artifacts with GC stress. See [artifact validation](validation-artifacts.md).

Implemented formatter slice: AST-aware layout over a lossless token/comment
stream, preserved spellings/parentheses/order, result reparse and lexical
invariant checking, idempotence tests, and CLI write/check/stdout modes. Corpus
contracts also run after formatting, artifact loading, and GC stress. See
[formatter validation](validation-formatter.md).

Implemented input slice: effect-checked `args` and `readLine`, independently
borrowed host capabilities, bounded physical UTF-8 records, typed EOF/errors,
zero-argument prelude function values, CLI argument separation, and source/artifact
input corpus cases. Wire version 2 adds input intrinsics and still loads version 1.
See [host input validation](validation-host-input.md).

Implemented robustness baseline: four coverage-guided frontend/formatter/compiler/
loader targets share stable replay oracles, with pinned isolated tooling and
saved campaign evidence. Linux fresh-process VM/GC measurements distinguish
logical counters, process RSS, structured VM limits, and host allocation failure.
See [fuzz validation](validation-fuzzing.md) and
[resource measurements](validation-resources.md). These bounded measurements do
not establish sandboxing, sustained fuzz coverage, or a VM-wide byte budget.

Implemented reference slice: an [implemented-language guide](reference.md) with
eight standalone examples. Checked-in contracts execute every LARP code fence
through normal/stress source runs, formatting, and independently loaded artifacts
in CI; see [reference validation](validation-reference.md).

Implemented build baseline: two distinct clean source/target trees from one
tracked commit produce identical release CLI bytes with controlled path
remapping. Both binaries execute source and loaded-artifact smoke programs.
The [reproducible-build check](reproducible-builds.md) runs in a separate hosted
CI job; its local evidence does not establish cross-machine reproducibility.

Implemented candidate packaging: complete tracked source plus the prebuilt CLI,
MIT, deterministic tar/gzip metadata, provenance/checksums, and real execution
after extraction. CI attaches a reviewable Linux x86-64 candidate; see
[distribution](distribution.md). This is not a versioned Release or a broad
platform compatibility claim.

Implemented platform gate: [explicit Ubuntu 24.04 build and Ubuntu 24.04/26.04
same-archive execution](platforms.md), with complete packaged conformance and
cross-job digest comparison. The matrix [passed for `c6083b0`](validation-platforms.md#hosted-exact-commit-matrix).
Each later candidate still requires successful exact-commit hosted evidence.

Implemented versioning baseline: [independent version identities and preview
compatibility rules](versioning.md), `--version --json`, real diagnostic/encoder
checks, and frozen historical artifacts executed by the current VM. Package
provenance and both compatibility hosts check the same advertised contracts.

Prepared first named candidate: `0.1.0-preview.1`, with
[release notes](releases/0.1.0-preview.1.md) and a machine-checked descriptor.
Packaging and both compatibility hosts bind those documents to the real CLI's
version and protocol capabilities. This prepares review material; it does not
publish a tag/Release or claim maintainer acceptance of the design.

Remaining: preview design acceptance/publication and additional host APIs.
The maintainer selected the
[MIT License](../LICENSE). Hosted compiler CI and
bounded fuzz campaigns passed on `cc1354d`; see
[the exact-commit evidence](validation-fuzzing.md#hosted-verification). Later
changes require their own results before being reported as passed.
The [readiness audit](preview-readiness.md) records the grouped-place grammar and
constant-control-flow corrections with their validation. JSON CLI diagnostics and exact
runtime-span contracts are implemented; see the [protocol](diagnostics.md).

Gate: all v0.1 corpus cases are executed by a real harness; no pending cases in
the declared release subset. Fuzz lexer/parser/loader and measure VM/GC resource
limits. Preserve the selected MIT license and choose supported platforms. Run CI on the maintainer-selected
GitHub remote; local checks do not prove hosted CI passed.

## Later RFCs

Modules/packages, user-defined generics, richer pattern matching and result
propagation, scenario validation groups, nested/first-class scenarios, scenario
object graphs, generalized effects, async, native backend, optimizer, debugger, LSP.
Keep the typed IR backend-neutral, but do not promise native ABI compatibility.
