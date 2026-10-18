# Host input API

Status: implemented M6 slice for process arguments and standard input. Files,
network access, clocks, randomness, environment variables, and arbitrary host
function registration remain deferred. These functions are ordinary prelude
values; they are not reserved syntax and user declarations can shadow them.

## Language surface

```larp
sidequest nextLine() -> baddie<text> uses(io, observe) {
    return readLine().cope(err => {
        yap(err);
        return no_bitches;
    });
}

main_character {
    args().forEach(argument => yap(argument));
    baddie<text> line = nextLine();
    yap(line.copium("EOF"));
}
```

| Function | Signature | Bound |
| --- | --- | --- |
| `args` | `fn() -> mfs<text>` | `uses(io)` |
| `readLine` | `fn() -> result<baddie<text>, text>` | `uses(io)` |

`args()` returns a fresh mutable array on every call. Changing that array never
changes the host's arguments or another call's result. It excludes the launcher,
source/artifact path, and LARP CLI options. Empty strings remain real arguments.
Subsequent array operations have their usual `read`/`write`/`abort` bounds.

`readLine()` reads one physical LF-delimited UTF-8 record. `win(some(text))`
contains its payload with LF and an immediately preceding CR removed. A blank
line is `win(some(""))`; EOF before any byte is `win(no_bitches)`. A final
unterminated record is returned once, then EOF. A lone CR at EOF is preserved.
NUL and other valid UTF-8 contents are retained; there is no trimming.

An invalid UTF-8 record yields `fumble("input is not valid UTF-8")` after consuming
that whole record; later records remain readable. An underlying read error yields
`fumble("input read failed")` and latches input failure. Subsequent calls return
the same error without touching the reader, so a consumed partial prefix cannot
silently become a new whole record. Interrupted reads retry. EOF is also latched.
Expected failures are result data; no exception or implicit unwrap is involved.

Both functions can be stored, captured, returned, and called through a function
value. Their `io` bound survives closures, callbacks, role dispatch, and extensions.
Creating a function value does not read input. Calling it requires `io`.
A callback cannot erase that bound; `sybau` suppresses only ordinary observations
and still consumes input when its body reads. Constructors and lazy initializers
cannot call host input, and scenario specialization rejects it transitively.

## CLI

```sh
cargo run -p larp-cli -- run examples/input-sum.larp -- total < scores.txt
cargo run -p larp-cli -- build examples/input-sum.larp -o /tmp/input-sum.larpbc
cargo run -p larp-cli -- run /tmp/input-sum.larpbc --gc-stress -- total < scores.txt
```

For input `2\n4\nbad\n`, this example prints `total\n6\n`. Its parser recovery
contributes zero for invalid numeric records. Arguments after the first `--`
are passed verbatim, including option-looking strings and additional `--` values.
Without the separator, only documented LARP runtime options are accepted.
All CLI arguments must be UTF-8. `check`, `build`, `dump`, and `fmt` never execute
host input. Source and artifact execution receive the same capabilities.

## Embedding

`larp_vm::Host::new(arguments, input)` accepts independent optional capabilities:
`Option<&[String]>` and `Option<&mut dyn BufRead>`. `Host::default()` denies both;
`Some(&[])` is granted empty arguments, distinct from denial. The VM does not
reach ambient `std::env` or `std::io::stdin` itself.

Pass the host to:

```rust,ignore
execute_with_host(module, stdout, inspection, &mut host, limits)
```

The existing `execute`, `execute_with_stats`, and `execute_with_observers` APIs
use a denied host. A type-correct module can therefore fail with `LARP-R010` when
its embedder has not granted a required capability. Declaring `io` describes a
requirement; it does not grant authority. The CLI explicitly supplies its argument
slice and stdin reader. Input position, EOF/error state, and consumed-byte budget
persist across executions that share one host. Construct a new host to start a
new budget; the caller remains responsible for its reader's current position.

The verifier checks intrinsic signatures/effects even in unreachable code.
Regional frames cannot carry `io`; additionally, the runtime rejects both host
intrinsics while a scenario is active, before reading or allocating. Capability
and resource failures are fatal host/runtime conditions independent of a source
`abort` bound, as with fuel or call-frame exhaustion. `fumble` is reserved for
ordinary read/UTF-8 failures.

## Resource and lifetime policy

| Resource | Limit |
| --- | --- |
| Argument count | 4,096 |
| Total UTF-8 argument bytes | 1 MiB |
| One physical input record | 64 KiB, including CR/LF bytes |
| Total consumed input per host | 1 MiB, including delimiters and invalid records |

Argument budgets are checked before copying values. Input buffers are consumed
only within the remaining line/total budgets. An exact-size final record at EOF
is valid. Exceeding a budget or failing line-buffer allocation raises `LARP-R006`,
returns no partial record, and latches resource exhaustion for that host. The
reader may already have buffered future bytes internally; budgets describe bytes
consumed by LARP, not kernel reads or embedder-side prefetch/storage.

Arguments use ordinary traced arrays. Allocation safepoints retain all VM roots,
including arrays captured by a saved built-in callback after receiver reassignment.
Returned lines are owned text inside ordinary result/option values and contain
no hidden host handles. A module/artifact never serializes stdin, arguments,
capabilities, or host state. [Wire version 2](bytecode-format-v2.md) only identifies
the two typed operations; the loader still verifies them independently.

VM fuel bounds instructions, not the duration of a blocking host call. The CLI
waits for interactive stdin when requested. An embedder requiring cancellation or
timeouts must supply an appropriate reader/execution environment. No process RSS,
wall-clock limit, asynchronous input, or I/O rollback is promised by this slice.
