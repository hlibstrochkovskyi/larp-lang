# Implementation status

The compiler currently implements algorithmic programs and nominal objects from
Specification v0.1. Programs are lexed, parsed, name/type/effect checked, lowered
into typed control-flow IR, compiled to stack bytecode, verified, and executed by LARP's VM.
No host `eval`, source-to-Python/JavaScript translation, or AST execution is used.

## Working features

- `mf`, `unc`, `geeked`, `verdict`, `text`, and unit-returning functions.
- Explicit local types, local `let` inference, scopes, assignments, compound assignments.
- Contextual integer literals, exact numeric widths, checked integer arithmetic,
  IEEE floating-point operations, text concatenation, comparisons, and `mogg`.
- Real short-circuit `&&`/`||`; constant folding preserves skipped computation.
- Functions with explicit signatures, forward calls, mutual recursion, return checks.
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
  when their element types do. Dynamic array equality is rejected. Indexed
  assignment validates its place before evaluating the right side.
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
- `if`/`else`, `locked_in while`, `npc loop`, `goon`, and `touch_grass`.
- Explicit finite `uses(...)` bounds checked through direct and indirect calls,
  including function variables, higher-order parameters, and captured functions.
  A function effect bound can widen; it cannot silently narrow.
- `yap`, always-active `no_cap`, `crashout`, and the `unc` prelude constant `fortnine`.
- Structured English diagnostics with stable codes and UTF-8 source spans.
- Control-flow bytecode verification, definite slot initialization, stack typing,
  function signature and effect checks; the VM accepts only a verified module.
- Verified closure capture layouts and immutable capture slots; total result-tag
  branching gives each successor the correct payload type.
- Nominal bytecode layout/access checks, immutable method receiver slots,
  constructor-only terminal allocation, and constructor effect restrictions.
- Non-moving mark/sweep object, array, and closure storage with generation-checked
  handles, explicit roots, stress collection, and execution statistics through
  the Rust VM API. Suspended collection callbacks and partial results remain rooted.

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

## CLI

```sh
cargo run -p larp-cli -- check examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp --fuel 100000
cargo run -p larp-cli -- run examples/callbacks.larp --gc-stress
cargo run -p larp-cli -- run examples/collections.larp --gc-stress
cargo run -p larp-cli -- run examples/objects.larp --gc-stress
cargo run -p larp-cli -- run examples/roles.larp --gc-stress
cargo run -p larp-cli -- dump examples/primes.larp
cargo run -p larp-cli -- dump examples/primes.larp --ir
cargo run -p larp-cli -- explain LARP-T041
```

`check` compiles and verifies without execution. `run` recompiles source and
executes in-memory bytecode. `--gc-stress` collects at every object/array/closure
allocation safepoint and may be combined with `--fuel`. `dump` emits disassembly
or typed IR. `build` and `fmt` explicitly report that they are unavailable. Input
is one UTF-8 file.
Compile/runtime failures exit with 1; invalid CLI arguments exit with 2.

## Not implemented yet

Scenario regions,
`larp/flex/canon/palantir`, lazy bindings, `sybau`, persisted artifacts,
modules, host I/O APIs, and formatting. These are tracked by the existing roadmap,
not removed from the project objective. Later syntax is explicitly rejected.

The scenario model remains independent and is not a substitute for compiling
scenario programs. GC manages objects, closures, and dynamic arrays, including
cycles between them. Option/result/fixed-array wrappers are owned values that
trace any contained handles. Role values trace their underlying object handle;
extracted methods retain that handle through closure captures. Text uses owned
immutable storage. Lazy cells and scenario roots need additional GC integration
when those features are implemented.

## Current limits and conservative checks

- Parser nesting: 128 guarded levels; each expression: at most 256 tokens.
- Type complexity: at most 128 nested edges and 4096 expanded nodes per type,
  counting repeated shared subtrees. The verifier preflights types across the
  entire module before forward calls can trigger structural comparisons.
- Function count: at most 4096 including main; per function: at most 65,536 local
  slots and 1,000,000 bytecode instructions.
- Verifier dataflow state: at most 4,000,000 stored type/initialization cells per
  function; operand stack: at most 65,536 entries. Resource-limit diagnostics are
  distinct from malformed-bytecode compiler bugs.
- Default execution budget: 1,000,000 instructions, adjustable with `--fuel`.
  VM call frames: at most 1024 by default; the Rust API accepts explicit limits.
- Integer operations on variables conservatively require `abort`, even if a more
  advanced range analysis could prove safety. Literal-only safe operations fold.
- Return-path checks do not prove constant conditions or loop invariants. A
  non-unit function needs syntactically complete reachable return paths or an
  unconditional non-returning loop/abort.
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
- Function types track effects but do not yet carry scenario-eligibility summaries.
  Region restrictions will need their own checks before scenarios can run.

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

See [roles and extensions validation](validation-roles-and-extensions.md) for
the current acceptance gate, and [objects validation](validation-objects.md) for
the preceding executed class checks. [Collections validation](validation-collections.md) and
[functions and recovery validation](validation-functions-and-recovery.md) record
preceding milestones.

`cargo test --workspace --locked --offline` exercises the frontend, compiler/VM,
bad bytecode, limits, diagnostics, and the independent model. Deterministic
malformed-input tests are included; they are not a substitute for sustained
coverage-guided fuzzing.

`python3 tools/run_conformance.py` executes every active fixture through `check`
and `run`, comparing stdout or primary diagnostic codes and source locations.
`--gc-stress` repeats execution with collection at every object/array/closure
allocation safepoint. Pending cases remain explicit. `--all` attempts the full corpus and currently fails on unimplemented features; this is not a complete v0.1 release.
