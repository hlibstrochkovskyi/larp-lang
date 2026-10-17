# Executable scalar core validation

Date: 2026-10-17. Environment: Linux, rustc/Cargo 1.99.0. No third-party Rust
dependencies, external compiler backend, or host language evaluation.

## Executed checks

- Workspace formatting and Clippy with warnings denied.
- Workspace tests: 45 passing tests across source locations, diagnostics, scalar
  operations, parsing, compiler/VM integration, bytecode verification, CLI, and
  the independent scenario model.
- Release compiler/VM integration tests: 14 passing tests, including overflow.
- Real CLI conformance: 11/11 active cases pass; 21 cases remain pending.
- Prime-number example: prints the 15 primes from 2 through 47.
- Recursive factorial example: prints 120. Loop example: prints 1009 and 10.
- Disassembly and typed IR tests confirm compilation artifacts precede execution.

The parser is tested with 2000 deterministic malformed token streams and deeply
nested/long expressions. It recovers from multiple malformed statements.
These tests are bounded robustness checks, not sustained fuzzing or a soundness
proof. Source spans are checked across Unicode text and CRLF input.

The bytecode verifier rejects wrong operand types, bad indices/targets, unknown
callees, uninitialized locals on a merge path, incompatible merge stacks, invalid
returns, and hidden effects, including structural/effect violations in unreachable
instructions. Its analysis allocation is bounded. The VM requires a verified
module and uses explicit frames rather than Rust recursion for LARP calls.

Compiler/VM tests cover short-circuiting without evaluating skipped invalid
arithmetic, type checks in dead code, i32/i64 minima, overflow, zero divisors,
minimum modulo -1, IEEE nonfinite values, left-to-right evaluation, lexical
shadowing, function effects, iteration boundaries, output failures, instruction
fuel, and recursion limits.

## Scope

M1's scalar compiler-to-VM gate is implemented. M2's algorithmic features work,
but options/results, recovery callbacks, checked conversions, and parseScore
remain unimplemented. Collections, OOP, GC, and language-level scenarios are
still pending. The full v0.1 goal is not complete.

Hosted CI is configured to run workspace checks and the active conformance
suite, but no remote has been configured and no hosted run is claimed.
