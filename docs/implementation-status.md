# Implementation status

The compiler currently implements an executable scalar subset of Specification
v0.1. Programs are lexed, parsed, name/type/effect checked, lowered into typed
control-flow IR, compiled to stack bytecode, verified, and executed by LARP's VM.
No host `eval`, source-to-Python/JavaScript translation, or AST execution is used.

## Working features

- `mf`, `unc`, `geeked`, `verdict`, `text`, and unit-returning functions.
- Explicit local types, local `let` inference, scopes, assignments, compound assignments.
- Contextual integer literals, exact numeric widths, checked integer arithmetic,
  IEEE floating-point operations, text concatenation, comparisons, and `mogg`.
- Real short-circuit `&&`/`||`; constant folding preserves skipped computation.
- Functions with explicit signatures, forward calls, mutual recursion, return checks.
- `if`/`else`, `locked_in while`, `npc loop`, `goon`, and `touch_grass`.
- Explicit finite `uses(...)` bounds checked through direct calls.
- `yap`, always-active `no_cap`, `crashout`, and the `unc` prelude constant `fortnine`.
- Structured English diagnostics with stable codes and UTF-8 source spans.
- Control-flow bytecode verification, definite slot initialization, stack typing,
  function signature and effect checks; the VM accepts only a verified module.

## CLI

```sh
cargo run -p larp-cli -- check examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp
cargo run -p larp-cli -- run examples/primes.larp --fuel 100000
cargo run -p larp-cli -- dump examples/primes.larp
cargo run -p larp-cli -- dump examples/primes.larp --ir
cargo run -p larp-cli -- explain LARP-T041
```

`check` compiles and verifies without execution. `run` recompiles source and
executes in-memory bytecode. `dump` emits disassembly or typed IR. `build` and
`fmt` explicitly report that they are unavailable. Input is one UTF-8 file.
Compile/runtime failures exit with 1; invalid CLI arguments exit with 2.

## Not implemented yet

Generic containers, options/results, checked conversion APIs, lambda/function
values, collection methods, classes, roles, extensions, scenario regions,
`larp/flex/canon/palantir`, lazy bindings, `sybau`, tracing GC, persisted artifacts,
modules, host I/O APIs, and formatting. These are tracked by the existing roadmap,
not removed from the project objective. Later syntax is explicitly rejected.

The scenario model remains independent and is not a substitute for compiling
scenario programs. No GC is claimed: current runtime values are scalar values
and owned immutable strings, so there are no language object cycles yet.

## Current limits and conservative checks

- Parser nesting: 128 guarded levels; each expression: at most 256 tokens.
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
- Indirect calls and function values await M3; no effect/region guarantees for
  those unimplemented features are claimed.

## Evidence

`cargo test --workspace --locked --offline` exercises the frontend, compiler/VM,
bad bytecode, limits, diagnostics, and the independent model. Deterministic
malformed-input tests are included; they are not a substitute for sustained
coverage-guided fuzzing.

`python3 tools/run_conformance.py` executes every active fixture through `check`
and `run`, comparing stdout or primary diagnostic codes and source locations.
Pending cases remain explicit. `--all` attempts the full corpus and currently
fails on unimplemented features; this is not a complete v0.1 release.
