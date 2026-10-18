# Contextual expression typing regressions

Date: 2026-10-19. This is a focused semantic correction, not approval of the
complete v0.1 specification or a developer-preview release.

## Reproduction and correction

Given `mf value = 7;` and `hediJeans<mf,1> fixed = [value];`, the compiler
accepted `[7] == fixed` but rejected `[value] == fixed` and `fixed == [value]`
with `LARP-T001`. String and nested literals also failed. The new execution
test reproduced these failures before the correction.

Binary operand checking used an array literal's default inferred `mfs` type
as if it were an established binding type. It rejected the mismatch before
contextual literal checking could select the fixed container type. Both sides
now prefer established operand types over default literal-container hints.
Ordinary expression checking still validates lengths, element types, and
equality eligibility. Runtime representation, bytecode, and evaluation order
are unchanged.

The [collection tests](../crates/larp-compiler/tests/collections.rs) cover both
operand orders for `==` and `!=`, variables, nested literals, strings, and empty
fixed arrays. They execute through the real compiler and VM with normal and
stress collection. Observable calls on each side verify left-to-right execution.
Negative cases retain length and element checks, reject existing dynamic/fixed
array comparisons, and reject equality for dynamic arrays and reference-bearing
fixed elements.

The active [regression fixture](../tests/conformance/pass/fixed-literal-equality.larp)
expects exactly `1\n2\nfr\n3\n4\nfr\n` on stdout.

## Explicit conversion result context

The next audit found another operand-order dependency:
`fraud<unc>(1) + 3_000_000_000` compiled, but
`3_000_000_000 + fraud<unc>(1)` rejected its literal as an overflowing `mf`.
`1 == fraud<unc>(1)` also failed, and context was missing through
`ascend<unc>(1).cope(handler)`. The conversion call's known result type was
absent from semantic hints. A focused VM test reproduced the failures first.

Hints now resolve the explicit target of the unshadowed conversion prelude:
`fraud<T>` supplies T, and `ascend<T>` supplies `result<T,ConversionError>`.
Existing recovery/member rules then preserve the payload type through `cope`.
Ordinary call checking still validates target/source types, arity, and effects.
The conversion argument does not inherit the neighboring operand's type.

The [function/recovery tests](../crates/larp-compiler/tests/functions_and_results.rs)
execute arithmetic, ordering, equality, unary negation, result constructors,
and recovery in both operand orders with normal/stress GC. They check output
order across two conversion operands, pure fallible recovery, and continued
rejection of implicit numeric conversions and erased abort effects. Local and
top-level declarations named `ascend`/`fraud` retain priority; explicit generic
calls to them remain unsupported. The existing
[conversion fixture](../tests/conformance/pass/conversions.larp) now includes
these contextual arithmetic/comparison/result assertions without changing its
expected stdout or increasing the corpus count.

## Local validation

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo test --workspace --locked --offline`: 373 tests passed.
- `cargo test -p larp-compiler --release --locked --offline`: 129 tests passed.
- `python3 tools/check_corpus.py`: 85 active definitions, no pending cases;
  this command alone does not execute programs.
- `python3 tools/run_conformance.py`: 85/85 passed.
- `python3 tools/run_conformance.py --gc-stress`: 85/85 passed.
- `python3 tools/run_conformance.py --artifacts --gc-stress`: 85/85 passed.
- `python3 tools/run_conformance.py --formatted --artifacts --gc-stress`: 85/85 passed.
- `python3 tools/run_reference.py --output artifacts/reference-conversion-context`:
  8 examples, 40 VM executions, and 113 CLI commands passed.
- `sh tools/git-local.sh diff --check`: passed.

These are local results. Hosted compiler/conformance and candidate-packaging
checks passed for the fixed-array correction in
[`8df4e78`](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39178524876).
Those checks do not establish hosted results for the later conversion correction.
No additional local fuzz campaign or coverage claim is part of this change.
