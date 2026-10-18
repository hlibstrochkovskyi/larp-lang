# Host input validation

Date: 2026-10-18. Scope: M6 arguments/stdin, explicit runtime capabilities, and
wire-version compatibility. This local acceptance report does not close every
language or developer-preview gate.

## Implemented slice

The prelude provides zero-argument `args` and `readLine` values with `io` bounds.
They lower through typed IR to verified intrinsics, then execute through an
explicit borrowed VM host. Arguments become fresh traced arrays. Lines become
ordinary result/option/text values. No ambient process access occurs in compiler
or VM libraries. The CLI supplies arguments after `--` and a stdin reader only
for `run`; legacy execution APIs deny both capabilities.

The [API contract](host-input.md) specifies EOF, record boundaries, error states,
budgets, and blocking behavior. The encoder writes [wire version 2](bytecode-format-v2.md),
while the loader retains version-1 input and independently verifies both. Host
capabilities and runtime input data are never embedded into bytecode artifacts.

## Executed behavior and boundaries

Compiler-to-VM tests execute direct functions, zero-argument builtin values,
saved array callbacks, role methods, bound role/extension methods, and callback
I/O. Argument mutation does not affect later `args()` results; saved receivers
retain the original array after reassignment. Runs use normal and stress GC and
assert that all managed allocations are reclaimed.

Tests reject effect erasure, indirect pure calls, I/O callbacks inside pure
functions, suppressed I/O inside pure functions, constructor reads, lazy reads,
and direct/helper/indirect/suppressed scenario input. A forged bytecode test
checks input effects even after a return. A private runtime guard test invokes
regional input with a reader that panics if touched and checks rejection before
host access or allocation. Legacy execution rejects input with `LARP-R010` and a
valid original source span. Inputs read before a scenario can become ordinary
value snapshots imported into the region; that positive path also executes.

Host reader tests cover blank records versus EOF, CRLF, final unterminated
records, lone final CR, NUL, invalid UTF-8 followed by a valid record, and valid
UTF-8 split across one-byte buffers. Interrupted reads retry; partial I/O failure
is latched without further reader access. Physical line and total-byte budgets,
exact-size EOF, repeated resource failure, and argument count/byte bounds are
exercised. A shared host preserves its cursor across separate VM executions.
Zero instruction fuel fails before consuming the first input record.

CLI tests pass Unicode/empty/option-looking arguments, distinguish runtime flags
from program arguments, format/build input programs, remove their source, and
execute the artifact with the same input. Invalid UTF-8 is recoverable result
data; an oversized physical record or too many arguments produces `LARP-R006`.
Check/build do not execute reads. The new input-sum example processes valid and
invalid numeric records through the real compiler/VM.

Artifact tests retain stable tags and truncation checks and add tags 20/21.
Version-1 old-tag modules still load; changing the header of a host-input artifact
to version 1 cannot enable the new operations. Decode returns a structured format
error before execution. Existing source maps and independent bytecode verification
remain required.

## Corpus and final local checks

The manifest is schema version 2, with optional declared UTF-8 stdin and argument
arrays on executable cases. Seven new active cases cover input, summation, EOF,
indirect effects, effect erasure, scenario rejection, and lazy rejection. All
subprocess runs receive explicit stdin rather than inheriting the test runner's
terminal. Formatted copies and loaded artifacts receive fresh identical inputs.

Passed checks:

- Workspace formatting and all-targets Clippy with warnings denied.
- 365 workspace tests; 125 compiler tests in release mode.
- Inventory validation: 84 active fixtures, 0 pending.
- All 84 cases through source `check`/`run`.
- All 84 cases through `--formatted --artifacts --gc-stress`, including exact
  stdout/inspection expectations or primary error codes/source locations.
- Git whitespace validation before commit.

CI runs the same combined path; local results alone do not establish a hosted
run. Blocking I/O is not preempted by VM fuel, and host byte limits do not bound
process RSS or embedder prefetch. Coverage-guided fuzzing, broader host APIs,
reference/distribution work, and license/platform choices remain open.
