# Grouped places and constant control-flow validation

Date: 2026-10-19. This report closes the two concrete G1/G2 findings from the
[preview audit](preview-readiness.md). It does not approve the complete proposed
specification or close the platform/distribution gates.

## Grouped assignment places

The grammar now recursively permits parentheses around a named place and its
member/index prefixes. This preserves existing compiler behavior while stating
its syntax explicitly. Grouping does not make an arithmetic expression, call,
array literal, fresh object, captured binding, or loop index writable.

One new frontend test checks AST targets and original spans. Four compiler tests
execute grouped local/self/member/fixed/dynamic places, compound assignment,
once-only index evaluation, sibling-preserving nested writes, and a bounds trap
before its RHS. Successful/trapping cases run with normal and stress GC.
The [grouped-place fixture](../tests/conformance/pass/grouped-places.larp) prints
exactly `1\n2\n3\n15\n99\n` through source, formatting and loaded artifacts.

## Constant conditions and return checking

The original compiler emitted both successors even for an established literal
condition. Execution tests reproduced missing-return and missing-initialization
errors before the correction. A shared branch builder now emits a single CFG
successor for a known scalar verdict. Both source branches still receive name,
type, access, effect and region checks before unreachable blocks are omitted
from bytecode. Regional call specialization scans all typed blocks.

The first range-bound comparison is separate from the runtime backedge.
Constant saved bounds can establish zero or at least one iteration; later
iterations compare the updated index. Dynamic bounds still execute once and
left-to-right. Integer-boundary ranges and finite loop termination retain their
existing regression checks.

Five [constant-flow tests](../crates/larp-compiler/tests/constant_flow.rs) exercise
functions, methods, roles, extensions, inferred/contextual lambdas, suppression
unwinding, short circuit, constructor early returns, loops and range entries.
Unknown conditional returns, reachable missing fields, empty-range returns,
dead type/access/effect violations, outer aliases and a transitive
scenario-ineligible helper are still rejected.

A final inferred-return compatibility check also validates earlier ordinary dead
returns against a settled lambda result. Expression-local CFG reachability
distinguishes these from a never-completing `return crashout(...)`; its internal
placeholder must not constrain result inference. Inferred lambdas with no
reachable return keep the existing no-useful-result convention. An infinite loop
does not need a return, but an actual execution still hits its explicit VM fuel
limit with a valid `LARP-R006` span.

Inference remains sequential and local. An earlier literal is not retroactively
retyped after a later return establishes a result; an explicit function type
supplies context from the start. Unknown mutable values and user functions are
not evaluated to prove conditions. This change adds neither general path solving
nor whole-body unification/replay.

## Executed local checks

- Formatting and Clippy with warnings denied passed.
- Workspace: 388 tests passed. Compiler release suite: 138 passed.
- Corpus inventory: 89 active definitions and no pending cases; inventory alone
  does not execute programs.
- All 89 cases passed in normal, stress, artifact/stress and
  formatted/artifact/stress modes, including JSON primary-span contracts.
- Eight reference examples passed 40 VM executions and 113 CLI commands;
  evidence remains in `artifacts/reference-control-flow-final/`.
- Independent review reran constant-flow 5/5, core 14/14, function/result 15/15,
  grouped-place 4/4 and IR integration 2/2 checks. Six additional stress CLI
  observations confirmed paired dead-return rejection, nonreturning-expression
  compatibility, range return/initialization and dynamic bound evaluation order.
- `sh tools/git-local.sh diff --check` passed.

The system Rust compiler reported a corrupt incremental cache during the first
full check and automatically rebuilt the affected artifacts. A fresh target
directory with `CARGO_INCREMENTAL=0` was used for independent Clippy/workspace/
release verification: Clippy had no warnings, and the same 388/138 test counts
passed. The clean CLI also passed all 89 combined formatted/artifact/stress
contracts. Fresh outputs remain in `artifacts/control-flow-clean-target/`.
Hosted results from earlier commits do not establish CI success for this change.
