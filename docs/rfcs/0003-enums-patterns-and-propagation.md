# RFC 0003 — Nominal enums, exhaustive match, and result propagation

Status: implemented development slice on `feature/patterns`, based on
`feature/projects` while PR #1 is under review. Local compiler/VM, frozen
version-3 and exact-commit hosted candidate checks passed as recorded in
[validation](../validation-patterns.md). Later revisions need their own checks.
The published preview does not implement this RFC. Toolchain version remains
`0.3.0-dev`; release publication is a separate decision.

## Motivation and syntax

Expected errors currently require a callback to inspect a result. Programs also
need their own finite alternatives without fake integer tags or parallel arrays.
Introduce nominal value enums, expression-oriented exhaustive matching and a
strict result propagation operator:

```larp
enum Funding {
    Bootstrapped,
    Angel(unc),
    Deal(text, unc),
}

sidequest runway(text input) -> result<unc, text> uses(abort) {
    mf score = parseScore(input)?;
    return win(fraud<unc>(score) * 10);
}

main_character {
    Funding funding = Funding::Angel(fortnine);
    unc money = match (funding) {
        Funding::Bootstrapped => 0,
        Funding::Angel(amount) => amount,
        Funding::Deal(_, amount) => amount,
    };
    yap(money);
}
```

The funding example is executed by the real compiler/VM in the M9 corpus and
[practical guide](../patterns.md); see the [validation record](../validation-patterns.md)
for the scope and remaining acceptance gates.
`enum` and `match` become reserved syntax words; programs that previously used
those names must rename them. `?` is postfix with call/member/index precedence,
so `f()?.field`, `items()[index]?`, and `nested()??` parse left-to-right.
The legacy grammar keeps `import` as an ordinary identifier while allowing
qualified enum constructors. There is no C-style enum-to-integer conversion.

## Enum declarations, identity, and payloads

Syntax: `enum NAME { VARIANT, VARIANT(TYPE, ...), ... }`, with optional trailing
comma. A declaration has at least one variant, at most 256 variants, and at most
64 payload fields per variant. Duplicate variant names are errors. Payloads
are positional; a nullary variant uses no parentheses. Each payload field is a
normal non-unit surface type; internal lazy cell types cannot be declared.

Enums are nominal: identical declarations in different source files remain
distinct. Declaration order does not restrict use. Enum dependencies through
payload/container/function types must form an acyclic type graph. Recursive
class references remain nominal and do not expand that graph; an enum may hold
a class reference. Recursive enum definitions require a later representation RFC.
Existing per-type expanded-node/depth budgets also include every variant payload.

An enum value contains its nominal identity, one discriminant and the active
payload values. Assignment copies this wrapper and value payloads; reference
payloads retain their identities, exactly as option/result/fixed-array copies do.
Variants have no mutable field access or implicit numeric discriminant access.
`E::V` constructs a nullary value. `E::V(args...)` constructs a payload variant,
evaluating arguments once from left to right with ordinary expected-type rules.
A payload constructor may be used as a first-class pure function with its concrete
signature; a nullary variant is a value, not an implicit zero-argument function.
Enums have no constructors, inherent methods, role implementation or inheritance;
ordinary concrete extensions can use enum receiver types.

A type supports structural equality only when all variant payload types support
it. Equality first compares the nominal identity and discriminant, then the
active fields. SnapshotValue eligibility requires every possible variant's
fields to be SnapshotValues. Managed references, closures and arrays are allowed
payloads; their containing enum is consequently not a SnapshotValue. This rule
must remain true through unused variants, functions, containers and bytecode.
Scalar `yap` eligibility does not expand to enums. Snapshot inspection displays
`E::V` or `E::V(field, ...)` with ordinary payload formatting.

In module syntax `highkey enum E` exports the type and all variants. Exported
payloads form part of the public interface and may not expose private nominal
types, including through containers/function signatures. `lib::E::V` accesses a
variant of an exported imported enum. Resolve the enum prefix as a type, never
as a callable chosen by argument count. A local enum name must not collide with
an import alias; report that ambiguity rather than silently changing existing
module value lookup. Variant names do not enter the unqualified value namespace.

## Patterns and coverage

`match (EXPRESSION) { PATTERN => BODY, ... }` is an expression. Its enclosing
statement still needs the usual semicolon. Bodies are expressions or ordinary
statement blocks. A normally completing block yields `unit`; there is no implicit
last-expression value or new `yield` syntax. `return` and `touch_grass` in an arm
retain their containing function/loop boundary and cleanup behavior.

Supported scrutinee types are `verdict`, `baddie<T>`, `result<T,E>`, and user enums.
Patterns are `_`, `fr`, `cap`, `no_bitches`, `some(binding)`, `win(binding)`,
`fumble(binding)`, and qualified enum variants with their payload binders. Each
payload position is an identifier or `_`; identifiers bind the exact payload
type in that arm only. Duplicate binders in one arm are errors; outer names may
be shadowed. Nested patterns, guards, ranges, or-patterns, destructuring arrays,
and matching arbitrary scalar literals require a later extension.

The built-in pattern constructors have fixed meaning determined by the
scrutinee type. A value/function named `win`, `some`, or `fumble` does not change
a pattern. User constructors always name their nominal type explicitly.

Every match must cover all alternatives, with a final `_` available as the
remaining-case pattern. Empty/nonexhaustive matches, duplicate constructor arms,
and arms that follow complete coverage are errors. There is no runtime
nonexhaustive-match trap. Coverage and duplicate checks are static even when a
scrutinee is constant. Every body still undergoes ordinary name/type/effect and
region checking; the language does not hide forbidden effects in dead arms.

The scrutinee is evaluated exactly once, before any arm. Only the selected body
executes. Payload extraction is by total typed dispatch, never an unchecked
projection or fabricated fallback. With an expected result type, every normally
completing body receives that context. Otherwise the first normally completing
body establishes the type in source order; later arms do not retroactively retype
earlier literals. Ordinary function-effect widening and nominal role coercions
apply where they already apply to expected values. Nonreturning bodies do not
constrain the merge type. All-nonreturning matches follow the existing
nonreturning-expression convention; they create no reachable merge value.

Match CFG edges must be connected before checking arm bodies, so containing
lambda return inference and definite initialization observe real reachability.
Typed payload adapter blocks may feed a shared wildcard body. Match arm and
payload counts are separately bounded; each child expression retains the normal
parser limits instead of charging all arms as one 256-token primary expression.

## Result propagation and return boundaries

`operand?` requires `operand: result<T,E>` and a containing return boundary with
result `result<U,E>`. Error types must agree exactly: no implicit conversion,
role coercion or function-effect widening changes `E`. Success yields `T`;
failure constructs `fumble(error)` with the enclosing result type and returns
from that boundary. `T` and `U` need not agree. There is no implicit wrapping of
ordinary return values in `win`, no option propagation, and no exception unwinding.

Sidequests and explicit lambdas own ordinary return boundaries. A contextually
typed lambda returning a known result type supports `?`; a lambda without a
known result context must request an explicit `fn(...) -> result<U,E>` type.
An inferred later return does not silently change this rule. `main_character`
and constructors cannot propagate. A direct propagation across a `larp` statement
boundary is rejected, like `return`; a region-local result-returning helper can
return ordinary result data without cancelling the surrounding scenario.

A `nonchalant result<U,E>` initializer deliberately owns a separate deferred
return boundary. Its `?` returns an error from the initializer, never from the
lexically enclosing source function. The error result is memoized like any other
SnapshotValue. Non-result lazy initializers reject propagation. Pure/abort-only
initializer effects and SnapshotValue captures/results remain mandatory. This
boundary is language semantics, not an accidental consequence of outlining.

Propagation evaluates the operand once in place, preserving all surrounding
left-to-right sequencing, short-circuiting and place/index validation. A failure
skips later arguments/operands/stores. It exits every local `sybau` scope it
crosses, while success retains the lexical suppression depth. Caller-inherited
suppression still follows ordinary frame return restoration. `?` itself adds no
`abort` or other effect; the operand retains all its normal effects.

## Compiler, bytecode, VM, and compatibility

Preserve the existing pipeline: source spans → AST → checked typed CFG → bytecode
→ mandatory independent verification → iterative VM. No AST evaluator or host
translation is introduced.

The shared type layer gains `Type::Enum(Arc<EnumType>)`, with canonical layouts
in `Program.enums` and `Module.enums`. Each descriptor contains an ID, display
name and ordered variants with their payload types. Context-free snapshot and
equality rules traverse the bounded payload graph. Before any type equality or
dataflow the verifier checks every descriptor against `Module.enums[id]`, including
all nested types and unreachable operations; a forged descriptor cannot claim a
reference-bearing nominal enum is snapshot-safe.

Construction uses typed `MakeEnum` IR/bytecode. `MatchSum { ty, targets }` is a
total dispatch for enum/option/result/verdict values. Its ordered alternatives
are declaration order for enums, `[no_bitches, some]`, `[win, fumble]`, and
`[cap, fr]`. The verifier supplies exactly the selected alternative's ordered
payload types to its successor. IR blocks support multiple typed parameters;
lowering stores payloads in reverse stack order. There is no unchecked unwrap.
The existing `MatchResult` operation remains accepted and supports `?` lowering.

Enum-owned fields are traversed when marking GC roots and heap edges, comparing
values and copying/formatting snapshots. Source and bytecode scenario checks
cover construction/dispatch explicitly, including unreachable instructions and
stack-only values. Non-snapshot enum payloads cannot bypass regional restrictions.

The writer advances to artifact wire **3**, retaining readers **1, 2 and 3**.
Old frames decode with an empty enum table; old tags/intrinsics remain stable.
New sections/tags are version-gated, bounded, and documented with the actual
codec. Type tables encode acyclic enum payload dependencies with earlier indices;
canonical enum declarations reference those checked descriptors. A new frozen
version-3 artifact must be executed before claiming reader 3 compatibility.
Existing version-1/2 fixtures remain byte-for-byte unchanged. Diagnostic and
version-info JSON schemas remain 1; new stable codes are registered normally.

## Acceptance gates

Execute the following through source, formatted source and independently loaded
artifacts, ordinary and stress GC, with original source files unavailable:

- Nullary, single/multi-payload enums; forward uses, functions, extensions,
  closures and managed-reference payloads; nominal module identity/privacy.
- Every built-in and user sum alternative, wildcard coverage, contextual numeric
  typing, nested closures over binders and block-arm control flow/initialization.
- Scrutinee evaluated once, unselected observations skipped, dead-arm effects
  checked, nonreturning arms and containing lambda inference handled correctly.
- `?` success/failure with different T/U, exact E, operand captures, chained postfix,
  left-to-right short-circuit and indexed place behavior, sybau cleanup, deferred
  memoized error results, and both allowed/forbidden regional boundaries.
- Reject incomplete/duplicate patterns, mismatched enum constructors, wrong arm
  types/arity, cyclic or excessive enum layouts, private payload leakage and
  forbidden effects/regions. Preserve precise primary/related source spans.
- Independently reject malformed enum metadata, descriptors, discriminants,
  payload stacks and dispatch edges in the verifier/loader without executing
  invalid bytecode. Preserve historical artifacts and existing corpus behavior.

Run required workspace formatting, Clippy, tests, corpus/conformance, reference,
README, module/project workflow, version, resource and reproducibility checks.
Packaged same-archive CI must pass on both declared Ubuntu hosts. Fuzz campaign
settings/seeds/targets are not expanded by this milestone. Record exact local and
hosted evidence in the PR before marking the whole M9 slice complete.

## References

The language design uses familiar sum-type and exhaustive-match ideas; these
references describe established designs rather than normative LARP behavior:

- [Rust enumeration items](https://doc.rust-lang.org/reference/items/enumerations.html).
- [Rust match expressions](https://doc.rust-lang.org/reference/expressions/match-expr.html).
- [Rust propagation expressions](https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-try-propagation-expression).

LARP's exact-error agreement, block-arm unit rule, finite initial patterns,
explicit lazy return boundary and regional restrictions are specified above.
