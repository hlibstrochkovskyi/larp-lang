# Contextual fixed-array equality regression

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

## Local validation

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo test --workspace --locked --offline`: 371 tests passed.
- `cargo test -p larp-compiler --release --locked --offline`: 127 tests passed.
- `python3 tools/check_corpus.py`: 85 active definitions, no pending cases;
  this command alone does not execute programs.
- `python3 tools/run_conformance.py`: 85/85 passed.
- `python3 tools/run_conformance.py --gc-stress`: 85/85 passed.
- `python3 tools/run_conformance.py --artifacts --gc-stress`: 85/85 passed.
- `python3 tools/run_conformance.py --formatted --artifacts --gc-stress`: 85/85 passed.
- `python3 tools/run_reference.py --output artifacts/reference-fixed-literal-equality`:
  8 examples, 40 VM executions, and 113 CLI commands passed.
- `sh tools/git-local.sh diff --check`: passed.

These are local results. Hosted checks on earlier commits do not establish
results for this correction. No additional fuzz campaign or coverage claim is
part of this change.
