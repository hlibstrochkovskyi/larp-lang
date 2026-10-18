# LARP practical reference

This guide describes the implemented developer-preview subset. See the
[specification](spec/v0.1.md) for exact rules and
[implementation status](implementation-status.md) for limits and deferred work.
Each `larp` block below is a complete, independently executable source file.
[Validation](validation-reference.md) records their real CLI execution.

## Build and run

Use the repository's Rust toolchain and save a sample as `program.larp`:

```sh
cargo build -p larp-cli --locked --offline
target/debug/larp check program.larp
target/debug/larp run program.larp
target/debug/larp run program.larp --fuel 1000000 --gc-stress
```

One UTF-8 file contains declarations and exactly one `main_character` entry.
Declarations may refer to later declarations. Braces delimit blocks; statements
end with `;`. Comments use `//` or non-nesting `/* ... */`. Identifiers use ASCII
letters, digits, and underscores; strings support Unicode and `\" \\ \n \r \t`.
There are no imports, mutable globals, user generics, inheritance, or async.

## Values, numbers, and control flow

| Type | Meaning |
| --- | --- |
| `mf`, `unc` | Signed 32-bit and 64-bit integers |
| `geeked`, `verdict` | IEEE binary64 and exactly `fr`/`cap` |
| `text`, `unit` | Immutable UTF-8 text and a function's absent useful result |
| `mfs<T>`, `hediJeans<T,N>` | Shared dynamic array and value-copied fixed array |
| `baddie<T>`, `result<T,E>` | Optional value and success/error value |

Use `T name = value;` or local `let name = value;`. Initialization is required;
ordinary locals are mutable. `let` infers from its initializer only. Integer
literals use an expected integer type, otherwise `mf`; `fortnine` is an `unc`
constant equal to 49. `fr`, `cap`, and `no_bitches` are values.

```larp
main_character {
    unc money = fortnine;
    locked_in while (money < 70) { money += 10; }
    yap(money);
    npc loop (mf i in 0..3) { yap(i); }
    if (money mogg 70) { yap(fr); } else { yap(cap); }
}
```

Output: `79\n0\n1\n2\nfr\n`. Ranges exclude their end; bounds are evaluated
once and the loop variable is read-only. `goon { ... }` loops indefinitely;
`touch_grass;` breaks the nearest loop. `return` exits a function.

Conditions require `verdict`: no integer/string truthiness. `mogg` means numeric
`>`. `&&`/`||` short-circuit; `!` negates a verdict. Arithmetic and comparisons
require identical numeric types: there is no implicit widening, narrowing, or
float/integer mixing. Integer overflow/division by zero abort in every build.
Division truncates toward zero; integer remainder has the dividend's sign;
minimum signed integer `% -1` is zero. `geeked` arithmetic follows IEEE rules,
including NaN/infinities; `%` is integer-only. `text + text`
concatenates. Text `.size()` counts Unicode scalars, not bytes/grapheme clusters.
Evaluation is left-to-right; assignment evaluates its place before its right side.

## Functions, captures, and collection callbacks

```larp
sidequest add(mf left, mf right) -> mf { return left + right; }

main_character {
    mf base = 10;
    let score = (mf bonus) => add(base, bonus);
    base = 100;
    yap(score(2));
    mfs<mf> scores = [2, 4, 49];
    scores.filter(x => x mogg 3).map(score).forEach(x => yap(x));
}
```

Output: `12\n14\n59\n`. Captures copy current values when a closure is created;
captured bindings cannot be reassigned. Captured arrays/objects still share
their identities. Lambda parameters need annotations or a contextual function
type; use `fn(mf) -> mf uses(abort)` for an explicit function type. Named functions
and prelude functions are first-class; polymorphic prelude values need context.
Non-unit functions require a value on every non-aborting return path.

`filter`, `map`, and `forEach` call callbacks eagerly, once per element,
left-to-right. `sorted()` returns a stable sorted copy for integer/text elements.
`push(value)` grows an array. Mutating an iterated receiver's length, including
through aliases/callbacks, aborts; indexed replacement remains allowed.

```larp
main_character {
    mfs<mf> values = [1, 2];
    let alias = values;
    alias[0] = 9;
    no_cap(values[0] == 9);
    hediJeans<mf, 2> fixed = [1, 2];
    hediJeans<mf, 2> copied = fixed;
    copied[0] = 9;
    yap(fixed[0]);
    yap(values.size());
}
```

Output: `1\n2\n`. Indices are `mf`; negative/out-of-range indexing aborts before
an assignment's right side executes. Fixed-array literals require exact length.
Nonempty unconstrained literals infer dynamic arrays; empty arrays require context.
Fixed arrays/options/results have structural equality if their payloads do.
Dynamic arrays and functions have no equality operator; objects compare identity.

## Effects are checked requirements

`sidequest f(...) -> T uses(read, abort)` declares an upper bound. Omitting
`uses(...)` means `uses(abort)`; `uses()` allows none of the five effects:

| Effect | Meaning |
| --- | --- |
| `read` | Read mutable heap through a receiver, parameter, or capture |
| `write` | Mutate that heap; does not grant `read` |
| `observe` | Ordinary/inspection output |
| `io` | Explicit external host capability |
| `abort` | Checked operation may terminate execution |

Local value reassignment and allocation alone are not semantic heap effects.
Integer operations on variables conservatively require `abort`. Bounds compose
through direct/indirect calls, callbacks, roles, and saved methods. Function
parameter/result types are invariant; an assigned value's outer effect bound may
widen, while nested/container function bounds remain invariant.
`main_character` permits all effects. Effects do not grant host authority or
promise immunity from OOM, fuel exhaustion, or host failures.

## Options, results, conversions, and failures

```larp
main_character {
    baddie<unc> funding = no_bitches;
    yap(funding.copium(20));
    result<mf, text> parsed = parseScore("unc");
    yap(parsed.cope(err => 49));
    result<mf, ConversionError> narrowed = ascend<mf>(funding.copium(20));
    yap(narrowed.cope(err => 0));
}
```

Output: `20\n49\n20\n`. Construct options with `some(value)`/`no_bitches`, and
results with `win(value)`/`fumble(error)`. `.isSome()`/`.isOk()` test the tag.
Payload types need enough context. `.copium(fallback)` evaluates its argument
eagerly even for a present value. `.cope(handler)` creates the handler eagerly,
but invokes it only for an error. There is no implicit unwrap or `?` operator.

`parseScore` accepts a whole signed decimal i32 string, without whitespace,
underscores, or leading `+`; failure is `fumble("invalid mf")`.
`ascend<mf/unc>` performs checked integer conversion and returns a result with
immutable `ConversionError.message`. `fraud<mf/unc>` instead aborts if conversion
fails. These support integer identity/widening/narrowing, not float conversion.
`no_cap(condition)` always asserts, including optimized builds. `crashout(text)`
always terminates; expected results do not unwind or invoke exception handlers.

## Objects, roles, and concrete extensions

```larp
role Scored { sidequest score() -> mf uses(read); }

archetype Counter : Scored {
    lowkey mf value;
    init(mf value) { self.value = value; }
    highkey sidequest score() -> mf uses(read) { return self.value; }
    highkey sidequest add(mf amount) -> unit uses(read, write, abort) {
        self.value += amount;
    }
}

sidequest mf.doubled() -> mf { return self * 2; }

main_character {
    Counter counter = new Counter(4);
    counter.add(2);
    Scored view = counter;
    fn() -> mf uses(read) saved = view.score;
    yap(saved().doubled());
}
```

Output: `12\n`. Classes have one `init`; all fields must be assigned before
successful construction. No partial `self` identity can escape or receive method
calls. Constructors cannot observe, perform I/O, or mutate existing objects.
Access defaults to `lowkey`; `highkey` is public. `self` cannot be reassigned.
Roles have signatures only. Implementations must explicitly name the role,
match types exactly, expose methods publicly, and fit its effect bounds.
Extensions resolve from the static receiver type and have no private access.
A real member wins by name, including when its access/signature causes an error.
Bound methods capture their receiver immediately and preserve effects/identity.

## Alternative state, lazy values, and suppressed output

```larp
archetype Founder {
    highkey unc valuation;
    highkey mf aura;
    init() { self.valuation = 20; self.aura = 0; }
}
main_character {
    Founder bro = new Founder();
    larp (bro) as Trial {
        flex bro.valuation = 1000000000;
        bro.aura += 2;
        canon bro.aura;
        bro.aura += 1;
        palantir(bro);
    }
    yap(bro.valuation);
    yap(bro.aura);
}
```

Stdout: `20\n3\n`. `palantir` separately reports visible baseline/performed fields,
last claims, and selection to stderr. `flex` writes performed state and records a
claim; ordinary writes do not erase it. `canon` selects the field's final value.
Normal fallthrough publishes selected values together; a trap discards the trial.
Already emitted output remains. An ordinary error result does not cancel commit.

Roots must be concrete classes containing only snapshot values: scalars/text,
ConversionError, or recursive option/result/fixed values of those. Outer snapshot
bindings import read-only copies. Nested regions, external I/O/reference graphs,
outer callbacks, exported regional values, and crossing returns/breaks are rejected.
Selective commit preserves field types, not application invariants.

```larp
sidequest announce() -> unit uses(observe) { yap("muted"); }
main_character {
    mf base = 4;
    nonchalant mf value = base * 2;
    base = 10;
    sybau { announce(); yap(value); }
    yap(value);
    yap(base);
}
```

Output: `8\n10\n`. `nonchalant` is read-only, captures snapshots at declaration,
computes on first read, and caches its value. Initializers permit only `abort`;
unused values do not evaluate. Capturing an existing lazy binding forces it then
copies its value. `sybau` suppresses `yap` dynamically across callees; arguments
still execute and effects remain checked. Inspection/diagnostics/I/O are not muted.

## Host input and persisted bytecode

```larp
sidequest next() -> baddie<text> uses(io, observe) {
    return readLine().cope(err => { yap(err); return no_bitches; });
}
main_character {
    args().forEach(argument => yap(argument));
    yap(next().copium("EOF"));
    yap(next().copium("EOF"));
    yap(next().copium("EOF"));
}
```

Run with `-- demo --literal` and stdin `hello\n\n`: output is
`demo\n--literal\nhello\n\nEOF\n`. `args()` returns a fresh array excluding CLI
options and the source path. `readLine()` returns `result<baddie<text>,text>`:
present text, distinct EOF, or a read/UTF-8 error. LF/CRLF are removed; a final
unterminated record remains valid. See [host input](host-input.md) for budgets,
latched failures, explicit embedding capabilities, and blocking behavior.

```sh
target/debug/larp build program.larp -o program.larpbc
target/debug/larp check program.larpbc
target/debug/larp run program.larpbc --gc-stress
target/debug/larp dump program.larpbc
target/debug/larp dump program.larp --ir
target/debug/larp fmt program.larp --stdout
target/debug/larp fmt program.larp --check
target/debug/larp explain LARP-T041
```

Artifacts embed full source text and are independently verified on loading;
original files are unnecessary for execution. They contain no input/capabilities.
`fmt` defaults to updating source; `--stdout` previews and `--check` checks layout.
Diagnostics have stable codes and source locations. Compile/runtime failures exit
1; usage errors exit 2. Default VM fuel is one million instructions, not a time or
memory bound. Managed GC does not make host allocation failure recoverable; see
[resource evidence](validation-resources.md). More complete programs live in
[examples](../examples/) and the [executed corpus](../tests/conformance/README.md).
