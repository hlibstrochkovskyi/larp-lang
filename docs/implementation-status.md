# Implementation status

The compiler currently implements algorithmic programs, nominal objects, and isolated scenarios from
Specification v0.1. Programs are lexed, parsed, name/type/effect checked, lowered
into typed control-flow IR, compiled to stack bytecode, verified, and executed by LARP's VM.
No host `eval`, source-to-Python/JavaScript translation, or AST execution is used.
The current public prerelease is [`v0.2.0-preview.1`](releases/0.2.0-preview.1-publication.md).
The [preview audit](preview-readiness.md) records resolved semantic regressions,
release evidence, and remaining design questions; a finite passing corpus and a
published preview do not approve the whole proposed specification. File modules
are included in this release as described below. This branch reports `0.3.0-dev`
and adds manifest-based project commands; dependency package management remains
future work. Exact-commit hosted validation of this development change is pending.

## Working features

- `mf`, `unc`, `geeked`, `verdict`, `text`, and unit-returning functions.
- Explicit local types, local `let` inference, scopes, assignments, compound assignments.
- Contextual integer literals, exact numeric widths, checked integer arithmetic,
  IEEE floating-point operations, text concatenation, comparisons, and `mogg`.
- Real short-circuit `&&`/`||`; constant folding preserves skipped computation.
- Functions with explicit signatures, forward calls, mutual recursion, return checks.
  Established constant conditions and initial range bounds select feasible
  return/initialization paths while dead branches still receive semantic checks.
  See [control-flow validation](validation-control-flow.md).
- First-class named functions and prelude functions; explicit `fn(...) -> T` types,
  contextual or annotated lambdas, nested closures, and returned closures.
- Immutable captures copied at closure creation; later outer reassignment does
  not change a saved capture. Function equality is rejected.
- `baddie<T>` with `no_bitches`, `some`, `.isSome()`, and eager `.copium(fallback)`.
- `result<T,E>` with `win`, `fumble`, `.isOk()`, and `.cope(handler)`. Handler
  creation is eager; invocation occurs only for an error. Option/result equality
  is structural when both payload types support equality.
- Contextual or inferred `mfs<T>` literals; contextual `hediJeans<T,N>` literals,
  checked `mf` indexing, indexed assignment, and `size()`. Dynamic arrays share
  identity; fixed arrays copy elements by value and support recursive equality
  when their element types do. Equality context types an array literal from
  either typed operand, including nested fixed literals. Dynamic array equality
  is rejected. Indexed assignment validates its place before evaluating the right side.
  See the [contextual equality regression](validation-contextual-equality.md).
- Dynamic array `push`, stable integer/text `sorted`, and eager `filter`, `map`,
  and `forEach`; callback effects propagate through typed built-in operations.
  Runtime guards reject structural mutation of an iterated receiver through any
  alias. Indexed replacement remains permitted.
- `text.size()` counts Unicode scalars.
- Nominal `archetype` classes with one `init`, mutable typed fields, default
  `lowkey` visibility, and `highkey` public members. Class values share non-null
  identity through aliases and support identity equality.
- Instance methods with an immutable `self` binding, direct calls, extracted
  bound methods, and private access retained by lexical nested closures.
- Constructor field initialization checked over control flow, including early
  `return;` paths and reads before assignment. No partial identity exists during
  initialization; terminal allocation publishes the completed object. Constructors
  cannot capture/use `self` as a value, call its methods, mutate existing heap
  state, observe, or perform I/O. Nested `new` is allowed.
- `parseScore(text)`, checked `ascend<mf/unc>`, aborting `fraud<mf/unc>`, and
  immutable `ConversionError.message`. Float/integer conversions are rejected.
  Explicit conversion results supply literal context in either operand order,
  including recovery through `.cope()`.
- `if`/`else`, `locked_in while`, `npc loop`, `goon`, and `touch_grass`.
- Explicit finite `uses(...)` bounds checked through direct and indirect calls,
  including function variables, higher-order parameters, and captured functions.
  A function effect bound can widen; it cannot silently narrow.
- `yap`, always-active `no_cap`, `crashout`, and the `unc` prelude constant `fortnine`.
- Structured English diagnostics with stable codes and UTF-8 source spans.
  `--diagnostic-format=json` exposes versioned JSON Lines on stderr, including
  exact labels, types/help, and encoded inspection events; see the
  [diagnostic protocol](diagnostics.md).
- Control-flow bytecode verification, definite slot initialization, stack typing,
  function signature and effect checks; the VM accepts only a verified module.
- Verified closure capture layouts and immutable capture slots; total result-tag
  branching gives each successor the correct payload type.
- Nominal bytecode layout/access checks, immutable method receiver slots,
  constructor-only terminal allocation, and constructor effect restrictions.
- Non-moving mark/sweep object, array, and closure storage with generation-checked
  handles, explicit roots, stress collection, and execution statistics through
  the Rust VM API. Suspended collection callbacks and partial results remain rooted.

## File modules (`0.2.0-preview.1`)

[File modules](modules.md) extend the single-file baseline with explicit relative
imports, private-by-default top-level declarations, `highkey` exports, and
`alias::name` references in expressions and type annotations. Each reachable
unit is parsed and checked, including unused declarations. Shared dependencies
retain one nominal identity; equal spellings from distinct files remain distinct.

A pure injected-resolver graph and the CLI's canonical filesystem provider are
separate boundaries. Explicit-file commands use the canonical entry's parent
as their root; development manifest commands use the canonical manifest parent.
Imports stay within the selected root, including after symlink resolution. Graph limits
are 64 files, 4096 import edges, 4 MiB aggregate source bytes, and 131072 lines.
Cycles, wrong entry placement, inconsistent resolver identities, and duplicate
aliases produce structured diagnostics. Public interfaces cannot expose private
nominal types, even nested in containers or callback types.

Dot extensions remain lexical. Exported extensions can be called explicitly as
`ext::name(receiver, ...)`, using exact static receiver matching, or saved as
function values with their receiver parameter and effect bounds intact.
Whole-project checking preserves ordinary constructor/member privacy, effects,
closure/lazy bindings, role identity, and scenario restrictions. One verified
artifact embeds every reachable source map; execution never reopens imports.
The formatter accepts library units without `main_character` and formats one
specified file at a time. See [RFC 0001](rfcs/0001-file-modules.md) for the contract
and [project fixtures](../tests/projects/README.md) for executable acceptance.
The [local validation record](validation-file-modules.md) documents the passed
gate and its limits; hosted results remain specific to their commit.

## Manifest-based projects (`0.3.0-dev`)

Implemented CLI paths include `larp new NAME`, schema-1 TOML manifests, nearest
ancestor discovery, explicit `--manifest-path`, and no-file `check`, `run`, and
`build`. The [project guide](projects.md) describes their actual contract.
Scaffolds include a working entry and library, README, and `.gitignore`; no Git
repository or license is created. Default artifacts go to the manifest root
under `target/NAME.larpbc`. Build errors preserve existing output.

Project mode allows imports between sibling directories inside the canonical
manifest root; explicit-file mode retains the entry-parent rule and never
discovers a manifest. Runtime cwd, stdin, argument handling, effects, resource
limits and source/artifact verification remain unchanged. `fmt` and `dump`
still require explicit files. There are no project dependencies, registry,
workspaces, test runner, or incremental build cache.

Manifest load/schema/path failures use `LARP-J001` through `J003`; scaffold
failures use `J004`. Readable parse/schema errors retain diagnostic spans. The
CLI uses a vendored upstream TOML parser rather than a custom TOML subset.
This implementation is not yet a published release; local results and hosted
checks must be tied to the exact commit independently of the M7 release record.

## Roles, extensions, and bound built-ins

The M4 feature set is implemented and exercised through the compiler and VM.
See [roles and extensions validation](validation-roles-and-extensions.md) for
executed checks, coverage, and limits of the completed local acceptance gate.

- Nominal `role` signatures and explicit `archetype C : R1, R2` implementations.
  Every required method must be public, match parameter/result types exactly,
  and stay within the role's effect bound. Roles have no fields, default methods,
  inheritance, or downcasts.
- Contextual class-to-role conversion where the class explicitly implements the
  expected role. The role view retains its underlying object's identity; calls
  dispatch through verified implementation tables. Bound role methods capture
  that view and preserve its declared interface effect bound.
- Concrete top-level receiver extensions resolved by the static receiver type.
  Real class/role/built-in members win by name. Inaccessible members and wrong
  signatures produce errors rather than falling back to an extension. Extensions
  have no class-private access; duplicate receiver/name declarations are rejected.
- Built-in method extraction: `size`, `push`, `sorted`, `isSome`, `isOk`, and
  `copium` infer their concrete function types. `map`, `filter`, `forEach`, and
  `cope` need an expected concrete function type and retain callback effect bounds.
  Extraction evaluates and captures its receiver immediately. Dynamic arrays and
  object/role views preserve shared identity; fixed arrays and tagged values are
  snapshots. Saved methods retain their original receiver after reassignment.

## Isolated scenarios

`larp (root) as Label` outlines a lexical region with a concrete value-only class
root. Entry copies its fields into baseline/performed storage and imports outer
SnapshotValue bindings as immutable copies. Other outer references and functions
are unavailable. The root is a non-exportable view; field reads/writes and
eligible method/helper calls operate on performed fields.

`flex` updates the performed value and last explicit claim. Ordinary writes do
not erase that claim. `canon` selects a visible field's final value and may appear
before its write. Only normal block fallthrough commits. The VM prepares the
complete next field vector and publishes it without callbacks or collection
between selected updates. Fatal errors discard the trial; observations remain.
`palantir` writes visible fields in declaration order to a separate inspection
sink (stderr in the CLI), with baseline, performed, last claim, and selection.

Region identity is explicit in IR and bytecode function metadata. All snapshot
locals in such a frame carry implicit `S_r(T)`; its sole concrete class denotes
`View_r(C)`. The compiler specializes the transitive call/closure graph per
lexical region. The verifier independently checks frame boundaries, signatures,
visibility, effect bounds, capture layouts, and every instruction, including
unreachable code. A regional value has no path into an ordinary result/store.
Value-only closures created inside may be called or passed to eligible helpers;
outer function bindings, reference captures/returns, reference allocation, role
dispatch, and unverified indirect targets are rejected.

Roots may contain scalar/text/ConversionError values and recursive
option/result/fixed SnapshotValues. Nested scenarios, root-bound method
extraction, copied first-class root bindings, crossing return/break, and external
I/O are rejected. Loops wholly inside a scenario may break. Ordinary error
results are data and do not cancel selected commit. Selective commit preserves
field types, not relationships between fields.

The M5 local acceptance suite passes; see [scenario validation](validation-scenarios.md).
The standalone state model is not used to execute these programs. See [scenario evaluation](scenario-evaluation.md)
for explicit-copy comparisons and the invariant counterexample.

## Memoized bindings and output suppression

`nonchalant T name = expression;` declares a read-only binding with a
SnapshotValue result. It captures SnapshotValue bindings by value at declaration,
and its initializer may only have the `abort` effect. The compiler retains its
actual effect bound for every later force. An unused initializer does not run;
the first read computes and caches the value, and later reads reuse it. A fatal
initializer terminates execution. Reentry reports `LARP-R009`.

Capturing another lazy binding forces that binding immediately and copies its
value. This also applies to ordinary closure captures, so capture itself may
abort even when the new closure or lazy initializer is never invoked. Cells are
an internal local representation: source function signatures, fields, containers,
and captures contain values, never exported lazy handles. A region may create
its own pure snapshot lazy binding; importing an outside lazy binding is rejected
with `LARP-S002`. Its initializer belongs to the same checked regional call graph.

`sybau { ... }` suppresses ordinary `yap` output dynamically, including direct,
indirect, role, extension, and callback calls made within it. Nested blocks nest
suppression. Return/break unwinds the scopes crossed; returning from a VM frame
restores its inherited depth. Suppressed calls still require `observe`, still
evaluate their arguments, and still execute their computation. `palantir`,
diagnostics, fatal errors, and external effects are not muted.

These features shipped in the first M6 preview; additional host APIs remain
future work. See [lazy and suppression validation](validation-lazy-and-suppression.md)
for the original measured feature checks and the
[publication record](releases/0.1.0-preview.1-publication.md) for release evidence.

## Host input

`args() -> mfs<text> uses(io)` returns a fresh array of program arguments.
`readLine() -> result<baddie<text>,text> uses(io)` returns a UTF-8 record, a
separate EOF value, or an ordinary read/UTF-8 error. Host argument/stdin permissions
are supplied independently to `execute_with_host`; legacy execution APIs deny
both with `LARP-R010`. A source effect does not itself grant authority.

I/O bounds survive named/prelude function values, callbacks, roles, extensions,
and suppression. Constructors/lazy initializers and transitive scenario graphs
reject input. The VM separately checks active regional state before host access.
Input and argument budgets produce `LARP-R006`; EOF/error/resource states persist
with the borrowed host. See [API and limits](host-input.md) and
[executed validation](validation-host-input.md).


## CLI

```sh
cargo run -p larp-cli -- check examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp --fuel 100000
cargo run -p larp-cli -- run examples/callbacks.larp --gc-stress
cargo run -p larp-cli -- run examples/collections.larp --gc-stress
cargo run -p larp-cli -- run examples/objects.larp --gc-stress
cargo run -p larp-cli -- run examples/roles.larp --gc-stress
cargo run -p larp-cli -- run examples/lazy.larp --gc-stress
cargo run -p larp-cli -- run tests/conformance/pass/scenario.larp --gc-stress
cargo run -p larp-cli -- dump examples/primes.larp
cargo run -p larp-cli -- dump examples/primes.larp --ir
cargo run -p larp-cli -- explain LARP-T041
cargo run -p larp-cli -- build examples/objects.larp -o /tmp/objects.larpbc
cargo run -p larp-cli -- run /tmp/objects.larpbc --gc-stress
```

`build SOURCE [-o OUTPUT]` writes deterministic versioned `.larpbc`; the default
is the source path with that extension. Artifacts embed complete source text and
validated spans. `check` compiles source or loads artifacts and verifies without
execution. `run` compiles source or independently verifies loaded bytecode, then
executes it. Original source files are unnecessary when running an artifact.
`run` accepts program arguments after `--` and supplies stdin only during
execution. `--gc-stress` collects at every object, array, closure, and lazy-cell allocation
safepoint and may be combined with `--fuel`. `dump` emits disassembly
or typed IR; `--ir` requires source because artifacts contain bytecode. Input is
a UTF-8 entry source (with optional file imports) or a versioned artifact. `fmt SOURCE [--check|--stdout]` formats
syntactically valid source without requiring type validity. It preserves raw
tokens/comments and checks its result before publication. Default mode writes
atomically; check/preview modes never write. See [formatter policy](formatter.md)
and the [artifact format](bytecode-format-v1.md) for baseline wire tables, limits, and output publication. The encoder now writes
[wire version 2](bytecode-format-v2.md), with version-1 loading retained.
Compile/runtime failures exit with 1; invalid CLI arguments exit with 2.

No-file project commands are available on this development branch:

```sh
larp new aura-app
larp check --manifest-path aura-app/larp.toml
larp run --manifest-path aura-app/larp.toml --gc-stress
larp build --manifest-path aura-app/larp.toml
```

From the project directory or its descendants, omit `--manifest-path` to discover
the nearest `larp.toml`. The published preview still requires explicit files.

## Not implemented yet

Dependency packages, re-exports, import cycles, separate compilation, and additional host
I/O APIs beyond arguments/stdin. These are tracked by the existing roadmap,
not removed from the project objective. Later syntax is explicitly rejected.

The scenario model remains independent and is not a substitute for compiling
scenario programs. GC manages objects, closures, dynamic arrays, and lazy cells,
including cycles among reference values. Option/result/fixed-array wrappers are
owned values that trace any contained handles. Role values trace their underlying object handle;
extracted methods retain that handle through closure captures. Text uses owned
immutable storage. Active scenario frames root the original object and their
snapshot/claim buffers. Active forcing frames root their lazy cell across nested
allocations, and memoized cells release captured initializer inputs.

## Current limits and conservative checks

- Parser nesting: 128 guarded levels; each expression: at most 256 tokens.
- Type complexity: at most 128 nested edges and 4096 expanded nodes per type,
  counting repeated shared subtrees. The verifier preflights types across the
  entire module, including complete parameter/capture/result headers, before
  forward calls can trigger structural comparisons.
- Function count: at most 4096 including main; per function: at most 65,536 local
  slots and 1,000,000 bytecode instructions.
- Verifier dataflow state: at most 4,000,000 stored type/initialization cells per
  function; at most 16,000,000 processed analysis cells across the module, including
  revisits; signature/type preflight: at most 16,000,000 expanded nodes plus
  nominal-name bytes, including derived intrinsic parameters; operand stack:
  at most 65,536 entries. Resource-limit diagnostics are
  distinct from malformed-bytecode compiler bugs.
- Default execution budget: 1,000,000 instructions, adjustable with `--fuel`.
  VM call frames: at most 1024 by default; the Rust API accepts explicit limits.
- Integer operations on variables conservatively require `abort`, even if a more
  advanced range analysis could prove safety. Literal-only safe operations fold.
- Return-path checks use established scalar conditions and initial range bounds.
  They do not prove general loop invariants or arbitrary algebraic identities.
  A non-unit function needs complete feasible return paths or a proven
  non-returning loop/abort; unknown conditions remain conservative.
- Function parameter and return types are invariant; only the top-level function
  effect bound may widen. Option/result payloads are invariant, including any
  nested function bounds.
- Lambda parameter types require annotations or an expected function type; result
  types are inferred locally. Bare `no_bitches`, `win`, and `fumble` require
  enough context to determine all payload types. Polymorphic prelude references
  such as `yap` require an expected concrete function signature.
- Function parameters, ordinary local bindings, and array elements cannot have
  type `unit`. Array literal operands are limited to 65,536, within the existing
  expression-token limit; runtime dynamic arrays may grow up to the `mf` length
  limit. Negative/out-of-range indices and oversized growth abort.
- Class and role declarations are each limited to 4096, with at most 65,536
  nominal member/table entries across the module. Constructor initialization analysis is bounded to 4,000,000
  block/field cells. Constructor calls conservatively carry `read,write,abort`;
  ordinary operations/callbacks with `write` bounds are rejected during
  construction, even when a particular call might only affect fresh storage.
- Function types retain effect bounds but do not by themselves prove scenario
  eligibility. Regional function instances form a closed verified graph; only
  value-signature closures created in that graph can reach an indirect call.
  Specialization shares the 4096 function limit with ordinary code and closures.
  Scenario imports and closure captures are SnapshotValues; no reference graphs,
  outer callbacks, function returns, or opaque host targets are admitted.

## Closure and recovery example

```larp
main_character {
    mf base = 10;
    let score = (mf bonus) => base + bonus;
    base = 100;
    yap(score(2));
    yap(parseScore("unc").cope(err => 49));
}
```

This prints `12\n49\n`: the closure retains its captured 10, and the failed
parse is recovered through a callback. Both paths compile through typed IR and
verified bytecode.

## Collection example

```larp
main_character {
    mfs<mf> scores = [2, 4, 49];
    scores.filter(x => x mogg 3)
        .map(x => x + 1)
        .forEach(x => yap(x));
}
```

This prints `5\n50\n`. Callback execution is eager and left-to-right. Capturing
an array retains its reference identity; capturing a fixed array copies its
value, and its captured elements cannot be reassigned. References stored inside
a fixed array still retain their shared identity.

## Evidence

See [formatter validation](validation-formatter.md) for source formatting and
[artifact validation](validation-artifacts.md) for the current M6 slice and
[lazy and suppression validation](validation-lazy-and-suppression.md) for the preceding one. See [scenario validation](validation-scenarios.md) and
[scenario evaluation](scenario-evaluation.md) for the current M5 acceptance gate, [roles and extensions validation](validation-roles-and-extensions.md)
for the preceding completed gate, and [objects validation](validation-objects.md) for
the preceding executed class checks. [Collections validation](validation-collections.md) and
[functions and recovery validation](validation-functions-and-recovery.md) record
preceding milestones.

`cargo test --workspace --locked --offline` exercises the frontend, compiler/VM,
bad bytecode, limits, diagnostics, and the independent model. Deterministic
malformed-input tests and shared fuzz oracles are included. Four short
coverage-guided campaigns completed locally; see [fuzz validation](validation-fuzzing.md)
for input counts, sanitizer limits, and the warmed-corpus qualification. They do
not replace sustained campaigns. [Resource measurements](validation-resources.md)
record fresh-process VM/GC behavior and distinguish structured limits from host
allocation failure; fuel and traced-object counters are not a memory sandbox.

`python3 tools/run_conformance.py` executes every active fixture through `check`
and `run`, comparing stdout, explicitly declared inspection output, or primary diagnostic codes and source locations.
`--gc-stress` repeats execution with collection at every object, array, closure,
and lazy-cell allocation safepoint. All 89 current fixtures are active; `--all`
selects the same complete inventory and passes. `--artifacts` also builds valid
programs, loads/verifies them, and compares execution with source; invalid source
must fail build without producing output. `--formatted` formats temporary copies,
checks idempotence, and compiles/executes the same contracts; it composes with
artifact loading and GC stress. A finite corpus with no pending
cases does not establish full language conformance. The published first preview
has separate [release evidence](releases/0.1.0-preview.1-publication.md).
