# Enums, matching, and result propagation

Status: implemented development features on **`0.3.0-dev`**. The implementation
candidate passed [local and exact-commit hosted checks](validation-patterns.md).
The current public release,
[`v0.2.0-preview.1`](releases/0.2.0-preview.1-publication.md), does not include
`enum`, `match`, or `?`. [RFC 0003](rfcs/0003-enums-patterns-and-propagation.md)
and its [grammar extension](spec/patterns.ebnf) define the contract.

Build the development CLI using [the source-build instructions](../README.md#build-from-source).
Save either complete program below as `program.larp`, then run:

```sh
larp check program.larp
larp run program.larp --gc-stress
larp build program.larp -o program.larpbc
larp run program.larpbc --gc-stress
```

These commands also work in a [manifest project](projects.md). Each `larp` fence
has an ordered contract in [pattern-cases.json](pattern-cases.json); the existing
reference driver checks source, formatted source, and independently loaded
artifacts using the real compiler and VM. Exact-commit hosted evidence is
required separately before this development slice is accepted for a release.

## Describe alternatives and handle expected errors

An enum is a nominal value type with named alternatives. A variant either has
no payload or declares positional payload types. Construction keeps contextual
numeric types, and a payload constructor is an ordinary first-class pure
function value. It does not change an enum into an integer.

```larp
enum Funding {
    Bootstrapped,
    Angel(unc),
    Deal(text, unc),
}

sidequest projectedRunway(text input) -> result<unc, text> uses(abort) {
    mf score = parseScore(input)?;
    return win(fraud<unc>(score) * 10);
}

main_character {
    fn(unc) -> Funding uses() angel = Funding::Angel;
    Funding funding = angel(fortnine);

    unc money = match (funding) {
        Funding::Bootstrapped => 0,
        Funding::Angel(amount) => amount,
        Funding::Deal(_, amount) => amount,
    };
    yap(money);

    result<unc, text> runway = projectedRunway("49");
    yap(match (runway) {
        win(value) => value,
        fumble(_) => 0,
    });
    yap(projectedRunway("unc").cope(error => 20));

    baddie<mf> bonus = some(7);
    mf score = match (bonus) {
        some(value) => value,
        no_bitches => 0,
    };
    yap(match (score mogg 3) {
        fr => score,
        cap => 0,
    });
}
```

Output: `49\n490\n20\n7\n`.

`projectedRunway` accepts a `result<mf, text>` from `parseScore`, while returning
`result<unc, text>`. On success, `?` yields the `mf` payload. On failure, it
returns a `fumble` from `projectedRunway` immediately. The success types may
differ; the error type must agree exactly. A successful return still needs the
explicit `win(...)`. The checked conversion is separate from propagation.
`.cope()` remains useful when a caller wants a local fallback rather than an
early return.

`match` evaluates its scrutinee once and executes only the selected arm. Every
alternative must be covered. A final `_` can cover the remaining alternatives;
an arm after complete coverage, a duplicate variant arm, or a missing case is
an error, even for a constant scrutinee. Bodies retain ordinary name, type,
effect, and regional checks, including branches that cannot execute.

## Pattern bindings and result types

Supported scrutinees are `verdict`, `baddie<T>`, `result<T,E>`, and user enums.
Patterns are `fr`, `cap`, `no_bitches`, `some(name)`, `win(name)`,
`fumble(name)`, enum variants, and `_`. Each payload position binds its exact
type or uses `_` to discard it. Bindings belong to one arm; different arms may
reuse names, and an arm may shadow an outer binding. Duplicate non-discard
binders in one pattern are errors.

The built-in pattern constructors have fixed meanings. A local function or
value named `win`, `fumble`, or `some` cannot change pattern behavior. A user
enum pattern names its type, such as `Funding::Angel(amount)`. A nullary
variant uses `Funding::Bootstrapped`, without parentheses.

The expected result type of a match supplies context to normally completing
arms. Without one, the first normally completing arm establishes the type in
source order; a later `unc` does not retroactively widen an earlier `mf` literal.
No implicit numeric conversions are added. Nonreturning arms do not constrain
the merged value. An ordinary block arm that completes normally has `unit`;
its final expression is not an implicit return value. Statements containing a
match still need the usual trailing `;`.

Use a block arm for statements, `return`, or `touch_grass`. Those exits keep
the surrounding function/loop boundary and its cleanup rules. Nested patterns,
guards, or-patterns, ranges, arbitrary scalar literal patterns, and array
patterns are deferred; an inner match can explicitly inspect another sum value.

## Payload identity, captures, and deferred failures

Enum assignment copies the tag and value payloads. Reference payloads continue
to share identity. Matching does not move or invalidate the original value.
A closure can capture an arm's binding, including a managed reference, under
the usual capture and effect rules.

```larp
enum Selection {
    Empty,
    Values(mfs<mf>),
}

main_character {
    mfs<mf> scores = [4, 9];
    Selection selected = Selection::Values(scores);
    match (selected) {
        Selection::Empty => {},
        Selection::Values(values) => { values.push(49); },
    };
    yap(scores.size());

    fn() -> mf uses(read) count = match (selected) {
        Selection::Empty => () => 0,
        Selection::Values(values) => () => values.size(),
    };
    yap(count());

    nonchalant result<mf, text> deferred = win(parseScore("unc")?);
    yap(deferred.cope(error => 7));
    yap(deferred.cope(error => 7));
}
```

Output: `3\n3\n7\n7\n`.

The array in the enum and `scores` share the same storage. The selected closure
retains that array after leaving the arm scope; `uses(read)` describes its
array-size read. Such reference payloads participate in tracing GC.

The lazy initializer owns its own return boundary. Its `?` returns an error
from that deferred computation, not from `main_character` or a lexically
surrounding sidequest. The error result is memoized like any other completed
SnapshotValue. Creating an unused binding does not run it. Initializers remain
pure/abort-only and may capture only SnapshotValues.

An enum is a SnapshotValue only if every possible variant payload is one.
`Selection` above is consequently not eligible, even when its active value is
`Selection::Empty`. The same all-variants rule applies to structural equality:
an enum containing a dynamic array has no equality operator. Printable scalar
eligibility does not expand; use a match to print selected payloads instead of
passing an enum directly to `yap`.

## Modules, boundaries, and compatibility

`highkey enum Funding` exports the type and all variants from a source module.
Use `finance::Funding` in type positions and `finance::Funding::Angel(value)`
for a variant. Equal spellings from different modules retain different nominal
identities. Public variant payload types form part of the exported interface
and cannot expose private nominal types. A local enum name cannot collide with
an import alias; rename one instead of relying on argument-count dispatch.

Result propagation requires a known enclosing `result<U,E>` return type.
Contextually typed result-returning lambdas support it. A lambda whose return
type is inferred without such context must supply an explicit function type;
a later return does not silently establish the propagation boundary. Ordinary
`main_character`, constructors, non-result lazy initializers, and direct exits
across a `larp` statement boundary reject `?`. A region-local helper returning
ordinary result data still follows the normal regional restrictions.

Propagation adds no effect itself. Its operand keeps all existing effects and
capability requirements. Failure skips later evaluation, including subsequent
call arguments and stores, and unwinds crossed local `sybau` scopes. Success
keeps the current suppression depth. No exception unwinding or unchecked unwrap
is introduced, and class/array/closure payloads cannot bypass scenario rules.

Enum declarations currently require an acyclic enum payload-type graph. Class
references remain nominal rather than expanding that graph. Enums have no
inherent methods, inheritance, roles, or user generic parameters; ordinary
concrete extensions may use enum receivers. Limits include 256 variants per
enum, 64 payload fields per variant, 257 match arms, 64 payload binders per
pattern, and the existing expanded type and syntax-depth budgets.

The development compiler writes [artifact wire version 3](bytecode-format-v3.md)
and reads versions 1, 2, and 3, with mandatory verification. The published
`0.2.0-preview.1` compiler writes version 2 and reads versions 1 and 2; it cannot
load a new version-3 artifact. Recompile retained sources with the intended
toolchain when switching versions. Diagnostic and version-info JSON schemas
remain 1. `enum` and `match` are newly reserved words, so older programs using
those identifiers must rename them.
