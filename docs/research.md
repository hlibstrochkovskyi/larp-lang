# Engineering research

Reviewed 2026-10-17. These are primary documentation, original research, and an
implementation author's book. Recommendations for LARP are our design inferences,
not endorsements or proofs supplied by the sources.

| Source | Relevant observation | LARP decision / limitation |
| --- | --- | --- |
| [Kotlin extensions](https://kotlinlang.org/docs/extensions.html) | Extensions use static receiver resolution; applicable members take priority. | Adopt static extensions and member priority, with a smaller no-overload rule. Do not copy Kotlin's full resolution system. |
| [Koka language book](https://koka-lang.github.io/koka/doc/book.html) | Effect types describe computations; callback effects can propagate through higher-order functions. | Start with finite effect bounds and special built-in propagation. No row-polymorphic user effects or handlers. |
| [Composable Memory Transactions, Harris et al.](https://www.microsoft.com/en-us/research/publication/composable-memory-transactions/) | Transaction composition and exceptions require a defined state model. | Use it as background. LARP's single-root selective field commit is not STM or a concurrent transaction protocol. |
| [Crafting Interpreters: Garbage Collection](https://craftinginterpreters.com/garbage-collection.html) | Mark/sweep follows reachable roots; temporary values and closures must be rooted. | Use explicit VM roots, a handle arena, and GC stress tests. Add scenario snapshots to the root set. |
| [Rust Compiler Development Guide: diagnostics](https://rustc-dev-guide.rust-lang.org/diagnostics.html) | Diagnostics need deliberate codes, labels, notes, and actionable help. | Separate structured diagnostic data, explanations, and rendering. A meme is a title, not the error contract. |
| [JVM Specification, class files and verification](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html) | Persisted bytecode requires structural and instruction-level validation. | Specify a small independent format and verifier before artifact loading. JVM complexity is not the target. |
| [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) and [2024 resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html) | Workspaces share package settings and lockfiles; virtual workspaces select a resolver explicitly. | Use a virtual Rust 2024 workspace with resolver 3 and a tracked lockfile. |

## Alternatives considered

- **LLVM first:** technically possible, but GC integration, ABI, debug metadata,
  and native deployment do not settle our main uncertainty: language semantics.
  Preserve a backend-neutral IR; postpone native code generation.
- **Transpile to Python/JavaScript:** mismatches the project's purpose and risks
  inheriting host overflow, coercion, object, and exception semantics.
- **Production AST interpreter:** quick feedback, but it does not exercise the
  selected artifact pipeline. Use an independent model only where it answers a
  narrow semantic question; build executable slices through bytecode.
- **Reference counting alone:** simple ownership of acyclic values, but object
  and closure cycles need additional handling. A basic tracing heap is a more
  direct fit for the proposed managed object language.
- **General refinements/effects/ownership immediately:** each can be a research
  project. Use explicit bounded restrictions, then measure rejection pain.
- **Third-party compiler skills:** none were needed or installed. Repository
  instructions and primary sources provide a transparent starting point.

## Evidence still needed

The state model tests mechanics only. We have not demonstrated that `larp` beats
an ordinary snapshot API, that the region checker is sound, that GC is correct,
or that bytecode will be fast. Those require implementation, negative tests,
stress/fuzz testing, and measured workloads. No performance or safety claim is
inferred merely from choosing Rust.
