# Design review

Status: engineering response to Design Brief v0.3, 2026-10-17. All language
choices below are proposals. The private brief remains unmodified and untracked.

## What holds together

The central identity is credible: a recognizable language with rigorous static
semantics and an optional, bounded model of hypothetical state. Bytecode is a
valid compilation target. A VM gives direct control over object identity,
scenario lifetime, diagnostics, and host capabilities without requiring a native
backend first. Nominal classes, roles, composition, and first-class functions
can serve CLI programs without class inheritance.

The vocabulary has a useful separation: syntax controls evaluation, library
names provide ordinary operations, and diagnostic nicknames decorate stable
error codes. `PeterThiel` should be a scenario label, not a compiler special case.
`fortnine` is a typed prelude constant. Reserving the entire meme inventory would
create compatibility costs with no semantic benefit.

## Corrections to the sketches

| Sketch or ambiguity | Proposed resolution |
| --- | --- |
| A function adds two `mf` values and returns `unc` implicitly | Reject it; use matching types or a checked explicit conversion. |
| A scenario writes a `lowkey` field from unrelated code | Reject it; scenario syntax does not bypass encapsulation. Use a public method. |
| `canon` might capture the value at the statement | It selects a field; the final performed value is committed at normal block completion. |
| A `return` might count as successful completion | Forbid transfer across a scenario boundary. Only reaching its closing brace commits. |
| Copying the root isolates every alias automatically | It does not. Rebind the root to a region view and reject other reference captures. |
| A pure lazy initializer can read mutable state safely | Reject such reads. Capture immutable value snapshots when the binding is created. |
| A pure function necessarily terminates | It need not; purity excludes observable state and I/O, not divergence or a fatal trap. |
| A returned `fumble` aborts a scenario | A result is data. It does not change control flow or cancel commit. |
| `unc` can accept any integer | It is exactly i64; overflow traps in every build mode. |
| `.size()` silently mixes integer widths | It returns `mf`, with a specified collection length limit. |

## Proposed decisions

1. Nominal reference classes with one definite-initializing constructor; no class
   inheritance. Explicit roles use dynamic dispatch. Member access is private by
   default. Fields have no custom setters in v0.1.
2. Built-in generic containers, invariant reference types, local bidirectional
   inference, and explicit API types. User-defined generics follow the core
   implementation rather than blocking it.
3. A single-threaded stack bytecode VM. Safe Rust handles into a traced heap,
   initially non-moving mark-and-sweep. RC alone does not collect object cycles.
4. A finite effect set and explicit function effect upper bounds. No algebraic
   handlers. Propagate effects through indirect calls and callbacks.
5. One scalar/value-only scenario root. No nested scenarios, arbitrary object
   graphs, asynchronous execution, reentrant host calls, or external I/O.
6. Region-qualified values inside a scenario cannot become ordinary outer values.
   Only explicit observation and selected root fields cross its boundary.

These decisions reduce implementation risk, but are not evidence that scenarios
are more useful than explicit snapshots. The model tests only commit mechanics.
Region checking and usefulness still require a compiler slice and comparative
examples.

## Why not first-class `scenario<T>` yet?

A first-class scenario value makes lifetime, escape, ownership, and delayed commit
part of the public type system immediately. A lexical block bounds all of these
and still tests the proposed benefit. Keep the runtime overlay representation
independent of syntax so a later API can reuse it. An explicit value snapshot is
the control case, not an enemy of the language design.

## Does selective commit preserve invariants?

No. Committing `balance` while discarding `reserved` may violate an application
invariant even if both fields retain their declared types. Atomicity means all
selected writes become visible together; it does not prove business consistency.
The first version has no post-commit hooks or arbitrary refinement predicates.
Examples must use independent fields. Later grouped commits or pure validators
need a separate RFC. This limitation is a release gate for real-world claims.

## Scope discipline

The core and object model are substantial even without scenarios. Build them in
short, testable compiler-to-VM slices. Prototype scenario mechanics early in a
separate model, but do not grow a second, dynamically typed production runtime.
Do not install a third-party skill just because it mentions compilers: the
available general tools and primary sources are sufficient for this foundation.

No LLVM integration, package registry, optimizer, FFI, concurrency, arbitrary
refinements, or user-defined effect handlers is required for the first release.
